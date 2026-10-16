//! The reader executable: a frame around [`window::draw`], which holds the
//! layout so it can be driven headlessly. This file owns only the platform
//! parts: the file dialog, the title bar and the run loop.

use eframe::egui::{self, Ui};
use steleofd_i18n::MessageId;
use steleofd_reader::App;
use steleofd_reader::ExportFormat;
use steleofd_reader::window;

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

impl eframe::App for Reader {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let requests = window::draw(ui, &mut self.app);
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

/// Ask the platform for a second revision and compare it against the open one.
/// Cancelling is not a failure, so nothing is reported; a comparison that
/// cannot run reports why.
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

/// Ask the platform where the export should go. A PDF is one file so the
/// platform names it; PNG writes one file per page, so it names a folder.
/// Cancelling is not a failure, so nothing is reported.
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
            let title = app.i18n().text(MessageId::ReaderExportPdfTitle);
            let mut dialog = rfd::FileDialog::new().set_title(&title);
            if let Some(stem) = app.path().and_then(|p| p.file_stem()) {
                dialog = dialog.set_file_name(format!("{}.pdf", stem.to_string_lossy()));
            }
            dialog.add_filter("PDF document", &["pdf"]).save_file()
        }
        ExportFormat::Png => {
            let title = app.i18n().text(MessageId::ReaderExportPngTitle);
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
fn print_current_view(app: &mut App) {
    let Some(pages) = app.take_print_request() else {
        return;
    };
    if pages.is_empty() {
        return;
    }
    app.print_to(pages, app.print_settings());
}
