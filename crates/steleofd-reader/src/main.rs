//! The reader executable: a frame around [`window::draw`], which holds the
//! layout so it can be driven headlessly. This file owns only the platform
//! parts: the file dialog, the title bar and the run loop.

use eframe::egui::{self, Ui};
use steleofd_i18n::MessageId;
use steleofd_reader::App;
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
