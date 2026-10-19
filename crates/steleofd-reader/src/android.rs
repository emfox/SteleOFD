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
    let still_open = eframe::run_native(
        "SteleOFD Reader",
        options,
        Box::new(|cc| {
            // The system face carries the Chinese labels, as on the desktop.
            steleofd_reader::fonts::install_cjk_fallback(&cc.egui_ctx);
            Ok(Box::new(Reader::with_app(App::new())))
        }),
    );
    if let Err(err) = still_open {
        // Logcat is the only place this is seen, so the report says the thread
        // and the reason rather than leaving a silent exit.
        log::error!(
            "android_main on {:?}: the window closed with {err}",
            thread_name()
        );
        return;
    }
    log::warn!(
        "android_main on {:?} returned: the event loop ended",
        thread_name()
    );
}

/// The library version the Java VM is told about when the library is loaded;
/// `android-activity` defines the activity, so exporting the symbol is enough.
#[unsafe(no_mangle)]
pub extern "C" fn JNI_OnLoad(
    _vm: *mut jni::sys::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> jni::sys::jint {
    log::info!("the library was loaded, on {:?}", thread_name());
    jni::sys::JNI_VERSION_1_6
}

/// Which thread a line was written on. A panic names a thread only when its
/// hook prints, and the activity's and the reader's threads are different.
fn thread_name() -> String {
    std::thread::current()
        .name()
        .map_or_else(|| "unnamed".to_owned(), str::to_owned)
}
