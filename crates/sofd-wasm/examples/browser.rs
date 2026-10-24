//! The browser bindings: the calls a page makes, written in Rust so the shapes
//! are visible without reading the generated JavaScript. The crate's JS boundary
//! exists only on `--target wasm32-unknown-unknown`, which is the check CI runs;
//! a page is what executes it, and `crates/sofd-reader/web/` is one.
//!
//! `cargo build -p sofd-wasm --example browser --target wasm32-unknown-unknown`

use sofd_wasm::OfdDocument;

fn main() {
    // In a page the bytes come from a file input, and this is where they arrive.
    // The example writes a document out with the testkit so the call below has
    // something to open; a page has no generator and reads a real file.
    let bytes = sofd_testkit::golden_book();

    let mut doc = match OfdDocument::open(&bytes) {
        Ok(doc) => doc,
        Err(err) => {
            // `JsError` carries a message the page can show; the engine's own
            // error type never crosses the boundary.
            web_sys_console_error(&format!("{err:?}"));
            return;
        }
    };

    let pages = doc.page_count();
    let info = doc.info().expect("info");
    web_sys_console_log(&format!(
        "{pages} page(s), facts as a plain object: {info:?}"
    ));

    // A raster for the canvas, at the resolution the display has, and a vector
    // for the DOM: the same page through the same flat list, two backends.
    let png = doc.render_png(0, 144.0).expect("png");
    web_sys_console_log(&format!("page 0 as PNG: {} bytes", png.len()));

    let svg = doc.to_svg(0).expect("svg");
    web_sys_console_log(&format!("page 0 as SVG: {} bytes", svg.len()));

    let hits = doc.search("Stele").expect("search");
    web_sys_console_log(&format!("search: {hits:?}"));
}

/// The console, reached without a crate: the page's own logging is what a
/// binding does, and a second dependency to print a line is not worth carrying.
fn web_sys_console_log(message: &str) {
    js_log("log", message);
}

fn web_sys_console_error(message: &str) {
    js_log("error", message);
}

#[cfg(target_arch = "wasm32")]
fn js_log(level: &str, message: &str) {
    use wasm_bindgen::JsValue;
    use wasm_bindgen::prelude::wasm_bindgen;
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = console, js_name = log)]
        fn console_log(message: &str);
        #[wasm_bindgen(js_namespace = console, js_name = error)]
        fn console_error(message: &str);
    }
    let _ = JsValue::NULL;
    match level {
        "error" => console_error(message),
        _ => console_log(message),
    }
}

/// On the host there is no console, so the line goes to stderr. The example
/// still has to build there, because `cargo build --workspace --examples` runs
/// on a desktop toolchain.
#[cfg(not(target_arch = "wasm32"))]
fn js_log(level: &str, message: &str) {
    eprintln!("[{level}] {message}");
}
