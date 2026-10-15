//! The reader window: four zones around one canvas, in the order
//! `docs/UI-SPEC.md` fixes. The window draws and asks; `App` decides.

use eframe::egui::{self, TextureOptions, Ui};
use steleofd_i18n::MessageId;
use steleofd_reader::viewer::{DroppedFile, Layout, MAX_DPI, MIN_DPI};
use steleofd_reader::{App, Canvas};

/// The resolution a page is shown at when fitted to the width.
const FIT_WIDTH_DPI: f32 = 96.0;
/// The resolution a page is shown at when the whole page is in view.
const FIT_PAGE_DPI: f32 = 72.0;
/// The widest a page is drawn, so a large window does not blur a small page.
const MAX_PAGE_WIDTH: f32 = 900.0;

/// The window state, boxed so `eframe` can own it.
struct Reader {
    app: App,
    /// The title last pushed to the window, so it is pushed only on a change.
    title: String,
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SteleOFD Reader")
            .with_inner_size([1100.0, 820.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SteleOFD Reader",
        options,
        Box::new(|_cc| Ok(Box::new(Reader::new()))),
    )
}

impl Reader {
    fn new() -> Self {
        Self {
            app: App::new(),
            title: String::new(),
        }
    }
}

/// The seven steps of one frame, in the order the specification requires.
impl eframe::App for Reader {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.resolve_intent(ui);
        self.draw_menu(ui);
        self.draw_toolbar(ui);
        self.draw_canvas(ui);
        self.draw_status(ui);
        self.sync_title(ui);
        if self.app.wants_quit() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl Reader {
    /// Step 1: consume the open request, then anything dropped on the window.
    ///
    /// A modal opened inside a layout pass corrupts it, hence not the handler.
    fn resolve_intent(&mut self, ui: &mut Ui) {
        if self.app.take_open_request() {
            open_with_dialog(&mut self.app);
        }
        let dropped: Vec<egui::DroppedFile> = ui.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            if let Err(err) = self.app.open_dropped(to_dropped(&file)) {
                self.app.set_error(err.message);
            }
        }
    }

    /// Step 3: push the document's name into the window title bar.
    fn sync_title(&mut self, ui: &mut Ui) {
        let title = self.app.title();
        if title == self.title {
            return;
        }
        self.title = title.clone();
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    /// Z1: document-level actions. Anything page-level belongs in the toolbar.
    fn draw_menu(&mut self, ui: &mut Ui) {
        egui::Panel::top("menu").show(ui, |ui| {
            ui.horizontal(|ui| {
                let open = self.app.i18n().text(MessageId::ReaderOpen);
                if ui.button(open).clicked() {
                    self.app.request_open();
                }
                let close = self.app.close_label();
                if ui
                    .add_enabled(self.app.is_open(), egui::Button::new(&close))
                    .clicked()
                {
                    self.app.close();
                }
                ui.separator();
                for (key, layout) in [
                    (MessageId::ReaderLayoutSingle, Layout::Single),
                    (MessageId::ReaderLayoutDouble, Layout::Facing),
                    (MessageId::ReaderLayoutContinuous, Layout::Continuous),
                ] {
                    let label = self.app.i18n().text(key);
                    if ui
                        .selectable_label(self.app.layout() == layout, label)
                        .clicked()
                    {
                        self.app.set_layout(layout);
                    }
                }
            });
        });
    }

    /// Z2: actions on the page on screen. Controls disable rather than hide.
    fn draw_toolbar(&mut self, ui: &mut Ui) {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.app.has_previous(), egui::Button::new("◀"))
                    .clicked()
                {
                    self.app.previous_page();
                }
                if ui
                    .add_enabled(self.app.has_next(), egui::Button::new("▶"))
                    .clicked()
                {
                    self.app.next_page();
                }
                ui.separator();
                let fit_width = self.app.i18n().text(MessageId::ReaderFitWidth);
                if ui.button(fit_width).clicked() {
                    self.app.set_dpi(FIT_WIDTH_DPI);
                }
                let fit_page = self.app.i18n().text(MessageId::ReaderFitPage);
                if ui.button(fit_page).clicked() {
                    self.app.set_dpi(FIT_PAGE_DPI);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut dpi = self.app.dpi();
                    let text = self.app.i18n().text(MessageId::ReaderZoom);
                    if ui
                        .add(egui::Slider::new(&mut dpi, MIN_DPI..=MAX_DPI).text(text))
                        .changed()
                    {
                        self.app.set_dpi(dpi);
                    }
                });
            });
        });
    }

    /// Z3: the only zone that rasterizes, and the only one that may.
    fn draw_canvas(&mut self, ui: &mut Ui) {
        match self.app.canvas() {
            Canvas::Empty => {
                let hint = self.app.drop_hint();
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new(hint).weak());
                });
            }
            Canvas::Failed { message } => {
                ui.colored_label(egui::Color32::from_rgb(180, 40, 40), &message);
            }
            Canvas::Pending => {}
            Canvas::Blank | Canvas::Page => self.draw_page(ui),
        }
    }

    /// Paint the cached page, then note it if the page turned out to be blank.
    fn draw_page(&mut self, ui: &mut Ui) {
        let page = self.app.current_page();
        let blank = self.app.canvas() == Canvas::Blank;
        let Some(rendered) = self.app.cached(page) else {
            return;
        };
        let image = rendered.image.clone();
        let aspect = image.height() as f32 / image.width().max(1) as f32;
        let width = ui.available_width().min(MAX_PAGE_WIDTH);
        let size = egui::vec2(width, width * aspect);
        let texture = ui.ctx().load_texture("page", image, TextureOptions::LINEAR);
        let source = egui::load::SizedTexture::new(texture.id(), size);
        egui::ScrollArea::both()
            .id_salt("canvas")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.image(source);
            });
        if blank {
            let notice = self.app.blank_notice();
            ui.label(egui::RichText::new(notice).weak());
        }
    }

    /// Z4: document and page on the left, and a failure when there is one.
    fn draw_status(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = self.app.error() {
                    ui.colored_label(egui::Color32::from_rgb(180, 40, 40), &err.message);
                } else {
                    let info = self.app.info_label();
                    if !info.is_empty() {
                        ui.label(egui::RichText::new(info).weak());
                        ui.separator();
                    }
                    ui.label(self.app.page_label());
                }
            });
        });
    }
}

/// Translate what the platform handed over into the reader's own type.
///
/// A path is preferred; the web backend has none and sends bytes instead.
fn to_dropped(file: &egui::DroppedFile) -> DroppedFile {
    match &file.path {
        Some(path) => DroppedFile::Path(path.clone()),
        None => DroppedFile::Bytes {
            name: file.name.clone(),
            bytes: file.bytes.as_deref().unwrap_or_default().to_vec(),
        },
    }
}

/// Ask the platform for a file, then open it.
///
/// `egui` 0.35 dropped its built-in dialog, so this goes through the platform.
fn open_with_dialog(app: &mut App) {
    let picked = rfd::FileDialog::new()
        .add_filter("OFD document", &["ofd"])
        .set_title(app.i18n().text(MessageId::ReaderOpen))
        .pick_file();
    let Some(path) = picked else {
        return;
    };
    if let Err(err) = app.open_path(&path) {
        app.set_error(err.message);
    }
}
