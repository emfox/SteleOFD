//! The reader executable: a frame around [`window::draw`], which holds the
//! layout so it can be driven headlessly. This file owns the platform parts
//! only, so a page keeps the frame and supplies its own entry in `web.rs`.

// A GUI app must not open a console window on Windows; the attribute is a
// no-op on the other systems, so it stays unconditional.
#![cfg_attr(windows, windows_subsystem = "windows")]

use eframe::egui::{self, Ui};
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
use steleofd_i18n::MessageId;
use steleofd_reader::App;
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
use steleofd_reader::ExportFormat;
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
use steleofd_reader::ReaderMessage;
use steleofd_reader::window;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_arch = "wasm32")]
mod web;

/// The window state, boxed so `eframe` can own it.
pub(crate) struct Reader {
    app: App,
    /// The title last pushed to the window, so it is pushed only on a change.
    title: String,
}

impl Reader {
    pub(crate) fn with_app(app: App) -> Self {
        Self {
            app,
            title: String::new(),
        }
    }
}

impl eframe::App for Reader {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let requests = window::draw(ui, &mut self.app);
        #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
        self.serve_platform_requests(&requests);
        self.sync_title(ui);
        if requests.quit {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl Reader {
    /// Push the document's name into the window title bar, when it changes.
    fn sync_title(&mut self, ui: &mut Ui) {
        let title = self.app.title();
        if title == self.title {
            return;
        }
        self.title = title.clone();
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Title(title));
    }
}

/// What the platform does on its own: choose a file, choose where one goes,
/// reach a printer. The web build has none of this, so the requests stay
/// pending there rather than being answered with a dialog that cannot open.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
impl Reader {
    fn serve_platform_requests(&mut self, requests: &window::WindowRequests) {
        if requests.open_dialog {
            open_with_dialog(&mut self.app);
        }
        if let Some(format) = requests.export {
            export_with_dialog(&mut self.app, format);
        }
        if requests.print {
            print_current_view(&mut self.app);
        }
        if requests.compare {
            compare_with_dialog(&mut self.app);
        }
    }
}

/// The platform asks for a second revision and compares it against the open
/// one. Cancelling is not a failure; a comparison that cannot run reports why.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn compare_with_dialog(app: &mut App) {
    let title = app.i18n().text(MessageId::CompareOpenSecond);
    let picked = rfd::FileDialog::new()
        .add_filter("OFD document", &["ofd"])
        .set_title(title)
        .pick_file();
    let Some(path) = picked else {
        return;
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            app.set_error(format!("{}: {err}", path.display()));
            return;
        }
    };
    if let Err(err) = app.compare_bytes(&bytes) {
        app.set_error(err.message);
    }
}

/// Ask the platform for a file, then open it.
///
/// `egui` 0.35 dropped its built-in dialog, so this goes through the platform.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn open_with_dialog(app: &mut App) {
    let picked = rfd::FileDialog::new()
        .add_filter("OFD document", &["ofd"])
        .set_title(app.text(ReaderMessage::Open))
        .pick_file();
    let Some(path) = picked else {
        return;
    };
    if let Err(err) = app.open_path(&path) {
        app.set_error(err.message);
    }
}

/// Ask the platform where the export should go. A PDF is one file so the
/// platform names it; PNG writes one file per page, so it names a folder.
/// Cancelling is not a failure, so nothing is reported.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn export_with_dialog(app: &mut App, requested: ExportFormat) {
    // Consuming the request also learns the page selection, which the window
    // does not own. Cancelling the dialog leaves the request cleared, so the
    // platform is not asked again on the next frame.
    let Some((format, pages)) = app.take_export_request() else {
        return;
    };
    debug_assert_eq!(format, requested, "the window reported a different format");
    let destination = match format {
        ExportFormat::Pdf => {
            let title = app.text(ReaderMessage::ExportPdfTitle);
            let mut dialog = rfd::FileDialog::new().set_title(&title);
            if let Some(stem) = app.path().and_then(|p| p.file_stem()) {
                dialog = dialog.set_file_name(format!("{}.pdf", stem.to_string_lossy()));
            }
            dialog.add_filter("PDF document", &["pdf"]).save_file()
        }
        ExportFormat::Png => {
            let title = app.text(ReaderMessage::ExportPngTitle);
            rfd::FileDialog::new().set_title(&title).pick_folder()
        }
    };
    let Some(destination) = destination else {
        return;
    };
    app.export_to(format, pages, destination);
}

/// Print what the reader is looking at. Neither `egui` nor `eframe` prints, so
/// the pages on screen are laid out and rasterized here; reaching a printer is
/// the desktop's own path, and a job that will not rasterize is reported.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn print_current_view(app: &mut App) {
    let Some(pages) = app.take_print_request() else {
        return;
    };
    if pages.is_empty() {
        return;
    }
    app.print_to(pages, app.print_settings());
}

/// The native entry point: a window, a file dialog and a run loop. Android is
/// started by the system and the browser by a page, so this is absent there.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SteleOFD Reader")
            .with_inner_size([1100.0, 820.0])
            .with_min_inner_size([640.0, 480.0]),
        renderer: renderer_from_args(),
        ..Default::default()
    };
    eframe::run_native(
        "SteleOFD Reader",
        options,
        Box::new(|cc| {
            // egui ships latin and cyrillic only, so the Chinese interface
            // needs a CJK face before the first frame draws.
            steleofd_reader::fonts::install_cjk_fallback(&cc.egui_ctx);
            Ok(Box::new(Reader::with_app(App::new())))
        }),
    )
}

/// The renderer a `--renderer` argument asks for, wgpu when it says nothing.
/// A driver that cannot make a Vulkan device cannot say so in advance, which
/// is why the choice is made at startup rather than at build time.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn renderer_from_args() -> eframe::Renderer {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = match arg.split_once('=') {
            Some(("--renderer", value)) => Some(value.to_owned()),
            Some(_) => None,
            None if arg == "--renderer" => args.next(),
            None => continue,
        };
        match value.as_deref() {
            Some("glow") => return eframe::Renderer::Glow,
            Some("wgpu") => return eframe::Renderer::Wgpu,
            // An unknown name is not worth refusing to start over: the window
            // is the point, and the default renderer is the one worth trying.
            _ => {}
        }
    }
    eframe::Renderer::default()
}

/// Neither of the other two builds has a `main` to start from.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}
