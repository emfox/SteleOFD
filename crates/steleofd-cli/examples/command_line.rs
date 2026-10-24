//! The engine as a command line tool: open a document, ask what it declares,
//! render a page, verify its signatures. The `steleofd` binary does this and
//! more; what this shows is the library calls underneath it, so a caller with a
//! batch job of their own can write it rather than shell out to ours.
//!
//! `cargo run -p steleofd-cli --example command_line -- <file.ofd> <out-dir>`

use std::path::PathBuf;

use steleofd_core::{Document, RenderOptions};
use steleofd_io::FileSource;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::env::args()
        .nth(1)
        .ok_or("usage: command_line <file.ofd> <out-dir>")?;
    let out = std::env::args()
        .nth(2)
        .ok_or("usage: command_line <file.ofd> <out-dir>")?;
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out)?;

    // `open` takes a source rather than bytes, so a large document is read from
    // disk as pages ask for it instead of being held whole in memory.
    let source = FileSource::open(&file)?;
    let mut doc = Document::open(&source)?;

    let pages = doc.page_count();
    println!("{file}: {pages} page(s), version {}", doc.version());
    for page in doc.display_list().pages.iter() {
        let b = page.page_box;
        println!("  page {}: {:.0} x {:.0} mm", page.index, b.w, b.h);
    }
    let missing = doc.missing_fonts();
    if !missing.is_empty() {
        // A face nobody has is drawn in a substitute, so the page still renders
        // and looks wrong. Reporting it is the caller's job, since only they
        // know which faces their machines carry.
        println!("  {} face(s) the reader does not have", missing.len());
    }

    for index in 0..pages {
        let png = doc.to_png(index, &RenderOptions::default())?;
        let path = out.join(format!("page-{index}.png"));
        std::fs::write(&path, png)?;
        println!("  wrote {}", path.display());
    }

    // Verification answers whether the proof holds and, separately, whether its
    // signer is one this caller trusts. An unsigned document reports
    // `MissingSignatures`, since a list of zero reads as all-passed.
    match doc.signatures() {
        Ok(refs) if refs.is_empty() => println!("  no signatures"),
        Ok(refs) => {
            for (sig, status) in refs.iter().zip(doc.verify_signatures()?) {
                println!("  signature {}: {status:?}", sig.base_loc);
            }
        }
        Err(steleofd_error::OfdError::Format(steleofd_error::FormatError::MissingSignatures)) => {
            println!("  no signatures")
        }
        Err(err) => return Err(err.into()),
    }
    Ok(())
}
