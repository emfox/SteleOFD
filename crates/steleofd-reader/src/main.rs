//! The reader window: a menu bar, the pages, and a status bar.

use eframe::egui::{self, TextureOptions, Ui};
use steleofd_i18n::MessageId;
use steleofd_reader::App;
use steleofd_reader::viewer::{Layout, MAX_DPI, MIN_DPI};

/// The resolution a page is shown at when fitted to the width.
const FIT_WIDTH_DPI: f32 = 96.0;
/// The resolution a page is shown at when the whole page is in view.
const FIT_PAGE_DPI: f32 = 72.0;
/// The widest a page is drawn, so a large window does not blur a small page.
const MAX_PAGE_WIDTH: f32 = 900.0;

/// The window state, boxed so `eframe` can own it.
struct Reader {
    app: App,
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
        Box::new(|_cc| Ok(Box::new(Reader { app: App::new() }))),
    )
}

impl eframe::App for Reader {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        if self.app.take_open_request() {
            open_with_dialog(&mut self.app);
        }
        ui.horizontal(|ui| self.draw_menu(ui));
        ui.horizontal(|ui| self.draw_status(ui));
        ui.separator();
        self.draw_pages(ui);
        if self.app.wants_quit() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl Reader {
    fn draw_menu(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui
                .button(self.app.i18n().text(MessageId::ReaderOpen))
                .clicked()
            {
                self.app.request_open();
            }
            if ui.button("✕").on_hover_text("Close the document").clicked() {
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
    }

    fn draw_status(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if let Some(err) = self.app.error() {
                ui.colored_label(egui::Color32::from_rgb(180, 40, 40), &err.message);
            } else if self.app.page_count() == 0 {
                ui.label(egui::RichText::new("SteleOFD").weak());
            } else {
                ui.label(self.app.page_label());
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
    }

    fn draw_pages(&mut self, ui: &mut Ui) {
        if self.app.page_count() == 0 {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("Open an .ofd document to begin").weak());
            });
            return;
        }
        self.draw_navigation(ui);

        let page = self.app.current_page();
        if let Err(err) = self.app.page_image(page) {
            ui.colored_label(egui::Color32::from_rgb(180, 40, 40), &err.message);
            return;
        }
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
            .id_salt("page")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.image(source);
            });
    }

    fn draw_navigation(&mut self, ui: &mut Ui) {
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
        });
    }
}

/// Ask the platform for a file, then open it. `egui` 0.35 dropped its built-in
/// dialog, so this goes through the platform rather than drawing a file
/// browser nobody would want to use.
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
