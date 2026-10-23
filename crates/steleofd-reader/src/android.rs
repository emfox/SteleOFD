//! The Android entry point. An Android app has no `main`: the system starts a
//! thread of its own and calls `android_main` on it, handing over the activity
//! the event loop needs. Everything after that is the same frame the desktop
//! uses. A device also has no file dialog, so a document arrives through the
//! intent that started the activity, read before the first frame.

use eframe::egui;
use jni::objects::{JObject, JString};
use jni::{Env, JavaVM, jni_sig, jni_str};
use steleofd_reader::{App, DroppedFile};

use crate::Reader;

/// Called by `android-activity` once the activity exists. `no_mangle` is what
/// makes the symbol reachable from the Java side.
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    // Read before the loop starts: the intent names the document the tap was
    // for, and the activity is the only handle on it.
    let handed_over = document_from_intent(&app);
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
        Box::new(move |cc| {
            // The system face carries the Chinese labels, as on the desktop.
            steleofd_reader::fonts::install_cjk_fallback(&cc.egui_ctx);
            let mut reader = App::new();
            if let Some(dropped) = handed_over {
                // Stored rather than opened: the first frame is the first place
                // a failure has a status bar to be written into.
                reader.hand_over(dropped);
            }
            Ok(Box::new(Reader::with_app(reader)))
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

/// The document the activity was started for, if it was started for one.
///
/// A tap arrives as `ACTION_VIEW` with a content URI, a share as `ACTION_SEND`
/// with the same, and the bytes are read now rather than kept as a path: the
/// sandbox behind the URI is gone once the activity leaves the front. Every
/// failure is logged and dropped: refusing to start would be worse than empty.
fn document_from_intent(app: &android_activity::AndroidApp) -> Option<DroppedFile> {
    match read_shared_document(app) {
        Ok(Some(document)) => Some(document),
        Ok(None) => {
            log::info!("the activity was started without a document");
            None
        }
        Err(err) => {
            log::error!("the document the activity was started for was not read: {err}");
            None
        }
    }
}

/// The read itself, separated so its failures can be reported in one place.
fn read_shared_document(
    app: &android_activity::AndroidApp,
) -> Result<Option<DroppedFile>, Box<dyn std::error::Error>> {
    // SAFETY: `android-activity` owns both pointers for as long as the
    // `AndroidApp` is alive, and it is borrowed for the whole of this call. The
    // VM outlives every local reference taken through it.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    // The callback form rather than a borrowed `Env`: the guards it makes live
    // inside the closure's stack frames, so everything read here is released
    // by the time this returns.
    vm.attach_current_thread(|env| read_intent(env, app.activity_as_ptr()))
}

/// The read, with an `Env` in hand.
fn read_intent(
    env: &mut Env<'_>,
    activity_ptr: *mut std::ffi::c_void,
) -> Result<Option<DroppedFile>, Box<dyn std::error::Error>> {
    // SAFETY: the activity pointer is a local reference owned by the app
    // object, used before `android_main` returns.
    let activity = unsafe { JObject::from_raw(env, activity_ptr.cast()) };

    let intent = env
        .call_method(
            &activity,
            jni_str!("getIntent"),
            jni_sig!(() -> android.content.Intent),
            &[],
        )?
        .l()?;
    if intent.is_null() {
        return Ok(None);
    }
    let Some(uri) = intent_data(env, &intent)? else {
        return Ok(None);
    };

    let bytes = read_uri(env, &activity, &uri)?;
    // `open_bytes` reads the document rather than trusting the name, so an
    // unhelpful provider name costs nothing but a label.
    let name = display_name(env, &activity, &uri).unwrap_or_else(|| "document.ofd".to_owned());
    log::info!("read {name} from the intent, {} bytes", bytes.len());
    Ok(Some(DroppedFile::Bytes { name, bytes }))
}

/// The URI an intent carries, whether it is a `VIEW` data URI or a `SEND`
/// extra. `None` when the intent was the launcher's, which has neither.
///
/// The locals are attached to the envelope's lifetime rather than to the
/// `Env` borrow, so the URI outlives the call that made it.
fn intent_data<'local>(
    env: &mut Env<'local>,
    intent: &JObject<'local>,
) -> Result<Option<JObject<'local>>, Box<dyn std::error::Error>> {
    let data = env
        .call_method(
            intent,
            jni_str!("getData"),
            jni_sig!(() -> android.net.Uri),
            &[],
        )?
        .l()?;
    if !data.is_null() {
        return Ok(Some(data));
    }
    // `ACTION_SEND` puts the item in `EXTRA_STREAM` instead of the data field,
    // so a share from a file manager would otherwise open an empty window.
    let extra = env.new_string("android.intent.extra.STREAM")?;
    let sent = env
        .call_method(
            intent,
            jni_str!("getParcelableExtra"),
            jni_sig!((name: java.lang.String) -> android.os.Parcelable),
            &[(&extra).into()],
        )?
        .l()?;
    if sent.is_null() {
        return Ok(None);
    }
    Ok(Some(sent))
}

/// The bytes behind a content URI, read through the activity's resolver.
fn read_uri(
    env: &mut Env<'_>,
    activity: &JObject<'_>,
    uri: &JObject<'_>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let resolver = env
        .call_method(
            activity,
            jni_str!("getContentResolver"),
            jni_sig!(() -> android.content.ContentResolver),
            &[],
        )?
        .l()?;
    let stream = env
        .call_method(
            &resolver,
            jni_str!("openInputStream"),
            jni_sig!((uri: android.net.Uri) -> java.io.InputStream),
            &[uri.into()],
        )?
        .l()?;
    if stream.is_null() {
        return Err("the provider would not open the document".into());
    }

    // Read in chunks into a buffer rather than asking the stream for its size:
    // a provider is free to report none, and the length of a shared document is
    // not a number a reader should trust in advance.
    let mut bytes = Vec::new();
    let chunk = [0u8; 64 * 1024];
    loop {
        let buffer = env.byte_array_from_slice(&chunk)?;
        let read = env
            .call_method(
                &stream,
                jni_str!("read"),
                jni_sig!((buffer: byte[]) -> int),
                &[(&buffer).into()],
            )?
            .i()?;
        if read <= 0 {
            break;
        }
        let taken = usize::try_from(read)?;
        bytes.extend_from_slice(&chunk[..taken]);
    }
    // The stream owns a file descriptor, and a reader that closed the document
    // without closing this would hold the provider open until the app exits.
    let _ = env.call_method(&stream, jni_str!("close"), jni_sig!(() -> void), &[]);
    Ok(bytes)
}

/// What the provider calls the document, for the status bar. A provider is
/// free to answer `null` or throw, and either way the document is still read.
fn display_name(env: &mut Env<'_>, activity: &JObject<'_>, uri: &JObject<'_>) -> Option<String> {
    let resolver = env
        .call_method(
            activity,
            jni_str!("getContentResolver"),
            jni_sig!(() -> android.content.ContentResolver),
            &[],
        )
        .ok()?
        .l()
        .ok()?;
    let cursor = env
        .call_method(
            &resolver,
            jni_str!("query"),
            jni_sig!(
                (
                    uri: android.net.Uri,
                    projection: byte[],
                    selection: java.lang.String,
                    selection_args: byte[],
                    sort_order: java.lang.String
                ) -> android.database.Cursor
            ),
            &[
                uri.into(),
                (&JObject::null()).into(),
                (&JObject::null()).into(),
                (&JObject::null()).into(),
                (&JObject::null()).into(),
            ],
        )
        .ok()?
        .l()
        .ok()?;
    if cursor.is_null() {
        return None;
    }
    let column = env.new_string("_display_name").ok()?;
    let index = env
        .call_method(
            &cursor,
            jni_str!("getColumnIndex"),
            jni_sig!((name: java.lang.String) -> int),
            &[(&column).into()],
        )
        .ok()?
        .i()
        .ok()?;
    let moved = index >= 0
        && env
            .call_method(
                &cursor,
                jni_str!("moveToFirst"),
                jni_sig!(() -> boolean),
                &[],
            )
            .ok()?
            .z()
            .ok()?;
    let name = if moved {
        let value = env
            .call_method(
                &cursor,
                jni_str!("getString"),
                jni_sig!((column: int) -> java.lang.String),
                &[index.into()],
            )
            .ok()?
            .l()
            .ok()?;
        if value.is_null() {
            None
        } else {
            let value = JString::cast_local(env, value).ok()?;
            value.try_to_string(env).ok()
        }
    } else {
        None
    };
    let _ = env.call_method(&cursor, jni_str!("close"), jni_sig!(() -> void), &[]);
    name.filter(|name| !name.is_empty())
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
