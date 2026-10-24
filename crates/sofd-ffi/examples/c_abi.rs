//! The C ABI from the other side of the boundary: open from bytes, ask what the
//! document declares, verify its signatures, take the document out as PDF. The
//! header a C consumer compiles against is `crates/sofd-ffi/include/`
//! `sofd.h`, and these are the calls it makes.
//!
//! `cargo run -p sofd-ffi --example c_abi -- <file.ofd>`

use std::ffi::c_char;
use std::ptr;

use sofd_ffi::{
    SofdInfo, SofdSignature, sofd_close, sofd_info, sofd_open, sofd_to_pdf, sofd_verify,
    sofd_version,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: c_abi <file.ofd>")?;
    let bytes = std::fs::read(&path)?;

    // Every fallible call writes its reason here, because a C caller has no
    // exceptions to catch. A NUL buffer is allowed and drops the message.
    let mut err = [0 as c_char; 256];
    let err_len = err.len();

    unsafe {
        let version = std::ffi::CStr::from_ptr(sofd_version());
        println!("library {}", version.to_string_lossy());
    }

    // The handle is opaque and owned by the caller: it stays alive until
    // `sofd_close`, and every returned pointer borrows from it.
    let doc = sofd_open(bytes.as_ptr(), bytes.len(), err.as_mut_ptr(), err_len);
    if doc.is_null() {
        let message = unsafe { std::ffi::CStr::from_ptr(err.as_ptr()) };
        return Err(format!("open failed: {}", message.to_string_lossy()).into());
    }

    let mut info = SofdInfo {
        version: ptr::null(),
        pages: 0,
        width_mm: 0.0,
        height_mm: 0.0,
        embedded_fonts: 0,
        signatures: 0,
    };
    let ok = sofd_info(doc, &mut info, err.as_mut_ptr(), err_len);
    if ok != 0 {
        let version = unsafe { std::ffi::CStr::from_ptr(info.version) };
        println!(
            "{} page(s) of {:.0} x {:.0} mm, version {}, {} embedded face(s), {} signature(s)",
            info.pages,
            info.width_mm,
            info.height_mm,
            version.to_string_lossy(),
            info.embedded_fonts,
            info.signatures
        );
    }

    // Call once for the size, again with a buffer that holds it. An unsigned
    // document reports zero rather than failing; `SIZE_MAX` is the error.
    let count = sofd_verify(doc, ptr::null_mut(), 0, err.as_mut_ptr(), err_len);
    if count == usize::MAX {
        let message = unsafe { std::ffi::CStr::from_ptr(err.as_ptr()) };
        eprintln!("verify failed: {}", message.to_string_lossy());
    } else if count == 0 {
        println!("  no signatures");
    } else {
        let mut found: Vec<SofdSignature> = (0..count)
            .map(|_| SofdSignature {
                location: ptr::null(),
                valid: 0,
                unchecked: 0,
                untrusted: 0,
            })
            .collect();
        sofd_verify(doc, found.as_mut_ptr(), count, err.as_mut_ptr(), err_len);
        for sig in &found {
            let location = unsafe { std::ffi::CStr::from_ptr(sig.location) };
            // `valid` false beside `unchecked` true means nothing was proved,
            // which is not the same as a mismatch and must not be reported as one.
            let verdict = match (sig.valid, sig.unchecked, sig.untrusted) {
                (1, _, 1) => "proof holds, signer not trusted",
                (1, _, _) => "valid",
                (_, 1, _) => "unchecked",
                _ => "tampered",
            };
            println!("  {}: {verdict}", location.to_string_lossy());
        }
    }

    let pdf_len = sofd_to_pdf(doc, ptr::null_mut(), 0, err.as_mut_ptr(), err_len);
    if pdf_len != usize::MAX {
        let mut pdf = vec![0u8; pdf_len];
        let written = sofd_to_pdf(doc, pdf.as_mut_ptr(), pdf_len, err.as_mut_ptr(), err_len);
        let out = std::path::Path::new(&path).with_extension("pdf");
        std::fs::write(&out, &pdf[..written.min(pdf_len)])?;
        println!("wrote {}", out.display());
    }

    // Closing null is a no-op, so a teardown path needs no check of its own.
    sofd_close(doc);
    Ok(())
}
