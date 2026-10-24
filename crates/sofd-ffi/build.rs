//! Regenerate the C header whenever the bindings change.
//! The header is committed so C consumers can vendor it without a Rust
//! toolchain, and refreshed here so it never drifts from the source.

use std::fs;
use std::path::Path;

fn main() {
    // The OUT_DIR copy serves consumers of this crate; the committed copy is
    // only touched when the bytes differ, so repeat builds stay clean.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it");
    let out_dir = std::env::var("OUT_DIR").expect("cargo sets it");

    let bindings = cbindgen::generate(&manifest_dir).expect("cbindgen reads the crate");
    let mut rendered = Vec::new();
    bindings.write(&mut rendered);
    let generated = String::from_utf8(rendered).expect("the header is UTF-8");

    let out_header = Path::new(&out_dir).join("sofd.h");
    fs::write(&out_header, &generated).expect("write the OUT_DIR header");

    let committed = Path::new(&manifest_dir).join("include/sofd.h");
    let stale = fs::read_to_string(&committed).map_or(true, |old| old != generated);
    if stale {
        fs::create_dir_all(committed.parent().expect("include dir")).expect("create include");
        fs::write(&committed, &generated).expect("refresh the committed header");
    }
}
