//! The window's zones, drawn against a `Ui` so they can run headlessly.
//! Everything here obeys `docs/UI-SPEC.md`: panels in a fixed order, the canvas
//! state asked of `App`, and no string drawn that the catalogue did not give.

use eframe::egui::{self, TextureOptions, Ui};
use steleofd_i18n::MessageId;

use crate::app::{App, Canvas, Fit};
use crate::viewer::{DroppedFile, Layout, MAX_DPI, MIN_DPI};

/// The widest a page is drawn, so a large window does not blur a small page.
pub const MAX_PAGE_WIDTH: f32 = 900.0;

/// What the window wants the host to do after a frame, if anything.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowRequests {
    /// The reader asked for a file; the host opens the platform dialog.
    pub open_dialog: bool,
    /// The reader wants the window closed.
    pub quit: bool,
}

/// Draw one frame into `ui`, and report what the reader asked for.
///
/// The specification's seven steps in order; nothing here reaches a platform.
pub fn draw(ui: &mut Ui, app: &mut App) -> WindowRequests {
    let mut requests = WindowRequests::default();
    // Step 1 collects finished rasters before anything is drawn, so a page the
    // worker finished during the last frame is on screen in this one.
    app.poll();
    take_drops(ui, app);
    draw_menu(ui, app, &mut requests);
    draw_toolbar(ui, app);
    draw_canvas(ui, app);
    draw_status(ui, app);
    if app.wants_quit() {
        requests.quit = true;
    }
    requests
}

/// Consume the open request and anything dropped on the window this frame.
fn take_drops(ui: &mut Ui, app: &mut App) {
    let dropped: Vec<egui::DroppedFile> = ui.input(|i| i.raw.dropped_files.clone());
    for file in dropped {
        if let Err(err) = app.open_dropped(to_dropped(&file)) {
            app.set_error(err.message);
        }
    }
}

/// Z1: document-level actions. Page-level ones belong in the toolbar.
fn draw_menu(ui: &mut Ui, app: &mut App, requests: &mut WindowRequests) {
    egui::Panel::top("menu").show(ui, |ui| {
        ui.horizontal(|ui| {
            let open = app.i18n().text(MessageId::ReaderOpen);
            if ui.button(open).clicked() {
                app.request_open();
                requests.open_dialog = true;
            }
            let close = app.close_label();
            if ui
                .add_enabled(app.is_open(), egui::Button::new(&close))
                .clicked()
            {
                app.close();
            }
            ui.separator();
            for (key, layout) in [
                (MessageId::ReaderLayoutSingle, Layout::Single),
                (MessageId::ReaderLayoutDouble, Layout::Facing),
                (MessageId::ReaderLayoutContinuous, Layout::Continuous),
            ] {
                let label = app.i18n().text(key);
                if ui.selectable_label(app.layout() == layout, label).clicked() {
                    app.set_layout(layout);
                }
            }
        });
    });
}

/// Z2: actions on the page on screen. Controls disable rather than hide.
fn draw_toolbar(ui: &mut Ui, app: &mut App) {
    egui::Panel::top("toolbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(app.has_previous(), egui::Button::new("◀"))
                .clicked()
            {
                app.previous_page();
            }
            if ui
                .add_enabled(app.has_next(), egui::Button::new("▶"))
                .clicked()
            {
                app.next_page();
            }
            ui.separator();
            let fit_width = app.i18n().text(MessageId::ReaderFitWidth);
            if ui
                .selectable_label(app.fit() == Some(Fit::Width), fit_width)
                .clicked()
            {
                app.set_fit(Fit::Width);
            }
            let fit_page = app.i18n().text(MessageId::ReaderFitPage);
            if ui
                .selectable_label(app.fit() == Some(Fit::Page), fit_page)
                .clicked()
            {
                app.set_fit(Fit::Page);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut dpi = app.dpi();
                // The catalogue entry carries a percent placeholder, so the
                // slider is labelled with the value already substituted.
                let text = app.zoom_label();
                if ui
                    .add(egui::Slider::new(&mut dpi, MIN_DPI..=MAX_DPI).text(text))
                    .changed()
                {
                    app.set_dpi(dpi);
                }
            });
        });
    });
}

/// Z3: the only zone that rasterizes, and the only one that may.
fn draw_canvas(ui: &mut Ui, app: &mut App) {
    // The canvas is the only place that knows how much room there is, so a fit
    // is resolved here and the worker asked for the resolution it produces.
    let available = ui.available_size();
    app.apply_fit((available.x, available.y));
    match app.canvas() {
        Canvas::Empty => {
            let hint = app.drop_hint();
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new(hint).weak());
            });
        }
        // The message itself belongs to Z4; stating it twice would put the
        // same sentence on screen twice. Z3 only says something went wrong.
        Canvas::Failed => {
            let notice = app.error_label();
            ui.colored_label(egui::Color32::from_rgb(180, 40, 40), &notice);
        }
        // The pages on screen are already rasterized or are on their way, so
        // both cases draw the same thing; the pending page is simply absent
        // from the cache this frame.
        Canvas::Pending | Canvas::Blank | Canvas::Page => draw_pages(ui, app),
    }
}

/// Paint every page the layout puts on screen, side by side or stacked.
fn draw_pages(ui: &mut Ui, app: &mut App) {
    let pages = app.visible();
    if pages.is_empty() {
        return;
    }
    let horizontal = app.layout() == Layout::Facing;
    let blank = app.canvas() == Canvas::Blank;
    egui::ScrollArea::both()
        .id_salt("canvas")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let layout = if horizontal {
                egui::Layout::left_to_right(egui::Align::Min)
            } else {
                egui::Layout::top_down(egui::Align::Center)
            };
            ui.with_layout(layout, |ui| {
                for page in pages {
                    draw_one_page(ui, app, page, horizontal);
                }
            });
        });
    if blank {
        let notice = app.blank_notice();
        ui.label(egui::RichText::new(notice).weak());
    }
}

/// Paint a single cached page, sized to the space a spread leaves for it.
fn draw_one_page(ui: &mut Ui, app: &mut App, page: usize, horizontal: bool) {
    let Some(rendered) = app.cached(page) else {
        return;
    };
    let image = rendered.image.clone();
    let aspect = image.height() as f32 / image.width().max(1) as f32;
    let available = if horizontal {
        // Two pages share the row, minus the item spacing between them.
        (ui.available_width() - ui.spacing().item_spacing.x) / 2.0
    } else {
        ui.available_width()
    };
    let width = available.clamp(1.0, MAX_PAGE_WIDTH);
    let size = egui::vec2(width, width * aspect);
    // One texture per page and resolution; the name carries both so the cache
    // and the texture cannot disagree.
    let name = format!("page-{page}");
    let texture = ui.ctx().load_texture(name, image, TextureOptions::LINEAR);
    let source = egui::load::SizedTexture::new(texture.id(), size);
    ui.image(source);
}

/// Z4: document and page on the left, and a failure when there is one.
fn draw_status(ui: &mut Ui, app: &mut App) {
    egui::Panel::bottom("status").show(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(err) = app.error() {
                ui.colored_label(egui::Color32::from_rgb(180, 40, 40), &err.message);
            } else {
                let info = app.info_label();
                if !info.is_empty() {
                    ui.label(egui::RichText::new(info).weak());
                    ui.separator();
                }
                ui.label(app.page_label());
            }
        });
    });
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
