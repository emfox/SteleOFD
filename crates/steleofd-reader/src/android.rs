//! The Android entry point. An Android app has no `main`: the system starts a
//! thread of its own and calls `android_main` on it, handing over the activity
//! the event loop needs. Everything after that is the same frame the desktop uses.

use eframe::egui;
use steleofd_reader::App;

use crate::Reader;

/// Called by `android-activity` once the activity exists. `no_mangle` is what
/// makes the symbol reachable from the Java side.
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    let options = eframe::NativeOptions {
        android_app: Some(app),
        viewport: egui::ViewportBuilder::default()
            .with_title("SteleOFD Reader")
            .with_min_inner_size([360.0, 480.0]),
        ..Default::default()
    };
    let started = eframe::run_native(
        "SteleOFD Reader",
        options,
        Box::new(|cc| {
            // The system face carries the Chinese labels, as on the desktop.
            steleofd_reader::fonts::install_cjk_fallback(&cc.egui_ctx);
            Ok(Box::new(Reader::with_app(App::new())))
        }),
    );
    if let Err(err) = started {
        log::error!("the reader could not start: {err}");
    }
}

/// The library version the Java VM is told about when the library is loaded;
/// `android-activity` defines the activity, so exporting the symbol is enough.
#[unsafe(no_mangle)]
pub extern "C" fn JNI_OnLoad(
    _vm: *mut jni::sys::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> jni::sys::jint {
    jni::sys::JNI_VERSION_1_6
}
