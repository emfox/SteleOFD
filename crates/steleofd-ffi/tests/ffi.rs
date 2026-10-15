//! Host-side coverage for the C ABI: every exported function, called the way a
//! C caller would, error paths included. The document shape comes from the
//! shared testkit, so this suite cannot drift from the others.

use std::ffi::{CStr, c_char};
use std::ptr;

use steleofd_ffi::*;
use steleofd_testkit::{SIGN_FILE, fixture_bytes};

/// A scratch error buffer standing in for the caller's, sized as C would.
type Errors = [c_char; 256];

/// The message the library left in the buffer, or an empty string.
fn message(errors: &Errors) -> String {
    unsafe { CStr::from_ptr(errors.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// Open the fixture, run the body, and close the handle afterwards.
fn with_doc<R>(signed: bool, body: impl FnOnce(*mut SteleofdDocument) -> R) -> R {
    let bytes = fixture_bytes(signed);
    let mut errors: Errors = [0; 256];
    let doc = steleofd_open(
        bytes.as_ptr(),
        bytes.len(),
        errors.as_mut_ptr(),
        errors.len(),
    );
    assert!(!doc.is_null(), "the fixture opens: {}", message(&errors));
    let result = body(doc);
    steleofd_close(doc);
    result
}

/// Fill a caller buffer through the probe-then-fill protocol.
fn filled(size: usize, fill: impl FnOnce(*mut u8, usize) -> usize) -> Vec<u8> {
    let mut buf = vec![0u8; size];
    let copied = fill(buf.as_mut_ptr(), buf.len());
    assert_eq!(copied, size, "the filling call reports the same size");
    buf
}

#[test]
fn version_reports_the_crate_version() {
    let reported = unsafe { CStr::from_ptr(steleofd_version()) };
    assert_eq!(reported.to_str().unwrap(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn open_reports_the_page_count_and_the_declared_facts() {
    with_doc(false, |doc| {
        assert_eq!(steleofd_page_count(doc), 1);

        let mut errors: Errors = [0; 256];
        let mut info = SteleofdInfo {
            version: ptr::null(),
            pages: 0,
            width_mm: 0.0,
            height_mm: 0.0,
            embedded_fonts: 0,
            signatures: 0,
        };
        let rc = steleofd_info(doc, &mut info, errors.as_mut_ptr(), errors.len());
        assert_eq!(rc, 0, "{}", message(&errors));
        assert_eq!(
            unsafe { CStr::from_ptr(info.version) }.to_str().unwrap(),
            "1.0"
        );
        assert_eq!(info.pages, 1);
        assert_eq!(info.width_mm, 210.0);
        assert_eq!(info.height_mm, 297.0);
        assert_eq!(info.embedded_fonts, 1, "the container ships one face");
        assert_eq!(info.signatures, 0);
    });
}

#[test]
fn open_rejects_bytes_that_are_not_a_container() {
    let garbage = b"not an ofd file";
    let mut errors: Errors = [0; 256];
    let doc = steleofd_open(
        garbage.as_ptr(),
        garbage.len(),
        errors.as_mut_ptr(),
        errors.len(),
    );
    assert!(doc.is_null(), "garbage is not a document");
    assert!(!message(&errors).is_empty(), "the failure explains itself");
}

#[test]
fn open_rejects_a_null_pointer_with_a_length() {
    let mut errors: Errors = [0; 256];
    let doc = steleofd_open(ptr::null(), 16, errors.as_mut_ptr(), errors.len());
    assert!(doc.is_null(), "a length without a pointer is refused");
    assert!(!message(&errors).is_empty(), "the failure explains itself");
}

#[test]
fn null_handles_are_tolerated_everywhere() {
    steleofd_close(ptr::null_mut());
    assert_eq!(steleofd_page_count(ptr::null()), 0);

    let mut errors: Errors = [0; 256];
    let mut info = SteleofdInfo {
        version: ptr::null(),
        pages: 0,
        width_mm: 0.0,
        height_mm: 0.0,
        embedded_fonts: 0,
        signatures: 0,
    };
    assert_eq!(
        steleofd_info(
            ptr::null_mut(),
            &mut info,
            errors.as_mut_ptr(),
            errors.len()
        ),
        -1
    );
    assert_eq!(
        steleofd_search(
            ptr::null_mut(),
            c"x".as_ptr(),
            ptr::null_mut(),
            0,
            errors.as_mut_ptr(),
            errors.len()
        ),
        usize::MAX
    );
    assert_eq!(
        steleofd_verify(
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            errors.as_mut_ptr(),
            errors.len()
        ),
        usize::MAX
    );
    assert_eq!(
        steleofd_render_png(
            ptr::null_mut(),
            0,
            96.0,
            ptr::null_mut(),
            0,
            errors.as_mut_ptr(),
            errors.len()
        ),
        usize::MAX
    );
    assert_eq!(
        steleofd_to_pdf(
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            errors.as_mut_ptr(),
            errors.len()
        ),
        usize::MAX
    );
    assert_eq!(
        steleofd_to_svg(
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            0,
            errors.as_mut_ptr(),
            errors.len()
        ),
        usize::MAX
    );
}

#[test]
fn info_refuses_a_null_output_pointer() {
    with_doc(false, |doc| {
        let mut errors: Errors = [0; 256];
        let rc = steleofd_info(doc, ptr::null_mut(), errors.as_mut_ptr(), errors.len());
        assert_eq!(rc, -1);
        assert!(!message(&errors).is_empty(), "the failure explains itself");
    });
}

#[test]
fn error_messages_truncate_without_overrunning() {
    let mut errors: Errors = [0; 256];
    // A short buffer with a sentinel past its end: the library may fill the
    // first few bytes and nothing beyond.
    let (head, tail) = errors.split_at_mut(8);
    head.fill(b'#' as c_char);
    tail.fill(b'!' as c_char);

    let garbage = b"not an ofd file";
    let doc = steleofd_open(
        garbage.as_ptr(),
        garbage.len(),
        head.as_mut_ptr(),
        head.len(),
    );
    assert!(doc.is_null());
    assert_eq!(head[7], 0, "the message stays NUL-terminated");
    assert!(
        head[..7].iter().all(|byte| *byte != b'!' as c_char),
        "the message fills only the room it was given"
    );
    assert!(
        tail.iter().all(|byte| *byte == b'!' as c_char),
        "nothing past the buffer was touched"
    );
}

#[test]
fn render_png_probes_then_fills() {
    with_doc(false, |doc| {
        let mut errors: Errors = [0; 256];
        let size = steleofd_render_png(
            doc,
            0,
            96.0,
            ptr::null_mut(),
            0,
            errors.as_mut_ptr(),
            errors.len(),
        );
        assert!(size > 1000, "a rendered A4 page is not a stray byte");

        let png = filled(size, |buf, cap| {
            steleofd_render_png(doc, 0, 96.0, buf, cap, errors.as_mut_ptr(), errors.len())
        });
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "PNG magic");
    });
}

#[test]
fn render_png_reports_a_short_buffer_without_failing() {
    with_doc(false, |doc| {
        let mut errors: Errors = [0; 256];
        let size = steleofd_render_png(
            doc,
            0,
            96.0,
            ptr::null_mut(),
            0,
            errors.as_mut_ptr(),
            errors.len(),
        );
        let mut small = vec![0u8; 8];
        let again = steleofd_render_png(
            doc,
            0,
            96.0,
            small.as_mut_ptr(),
            small.len(),
            errors.as_mut_ptr(),
            errors.len(),
        );
        assert_eq!(again, size, "a too-small buffer still reports the size");
        assert!(small.iter().all(|byte| *byte == 0), "and writes nothing");
    });
}

#[test]
fn render_png_keys_the_cache_on_page_and_dpi() {
    with_doc(false, |doc| {
        let mut errors: Errors = [0; 256];
        let mut probe = |dpi: f64| {
            steleofd_render_png(
                doc,
                0,
                dpi,
                ptr::null_mut(),
                0,
                errors.as_mut_ptr(),
                errors.len(),
            )
        };
        let at_96 = probe(96.0);
        assert_eq!(probe(96.0), at_96, "the same request hits the cache");
        assert!(probe(48.0) < at_96, "a lower dpi renders a smaller page");
        assert_eq!(probe(96.0), at_96, "the cache holds one entry, keyed");
    });
}

#[test]
fn render_png_rejects_a_page_out_of_range() {
    with_doc(false, |doc| {
        let mut errors: Errors = [0; 256];
        let size = steleofd_render_png(
            doc,
            5,
            96.0,
            ptr::null_mut(),
            0,
            errors.as_mut_ptr(),
            errors.len(),
        );
        assert_eq!(size, usize::MAX);
        assert!(!message(&errors).is_empty(), "the failure explains itself");
    });
}

#[test]
fn to_pdf_probes_then_fills() {
    with_doc(false, |doc| {
        let mut errors: Errors = [0; 256];
        let size = steleofd_to_pdf(doc, ptr::null_mut(), 0, errors.as_mut_ptr(), errors.len());
        let pdf = filled(size, |buf, cap| {
            steleofd_to_pdf(doc, buf, cap, errors.as_mut_ptr(), errors.len())
        });
        assert!(pdf.starts_with(b"%PDF"), "PDF magic");
    });
}

#[test]
fn to_svg_probes_then_fills() {
    with_doc(false, |doc| {
        let mut errors: Errors = [0; 256];
        let size = steleofd_to_svg(
            doc,
            0,
            ptr::null_mut(),
            0,
            errors.as_mut_ptr(),
            errors.len(),
        );
        let svg = filled(size, |buf, cap| {
            steleofd_to_svg(doc, 0, buf, cap, errors.as_mut_ptr(), errors.len())
        });
        let text = String::from_utf8(svg).expect("SVG text");
        assert!(text.contains("<svg"), "an SVG document");
        assert!(text.contains("210") && text.contains("297"), "page size");
    });
}

#[test]
fn search_probes_then_fills() {
    with_doc(false, |doc| {
        let mut errors: Errors = [0; 256];
        let needle = c"steleofd";
        let count = steleofd_search(
            doc,
            needle.as_ptr(),
            ptr::null_mut(),
            0,
            errors.as_mut_ptr(),
            errors.len(),
        );
        assert_eq!(count, 1, "the page carries the text once");

        let mut hits: [SteleofdHit; 4] = std::array::from_fn(|_| SteleofdHit {
            page: 0,
            snippet: ptr::null(),
        });
        let again = steleofd_search(
            doc,
            needle.as_ptr(),
            hits.as_mut_ptr(),
            hits.len(),
            errors.as_mut_ptr(),
            errors.len(),
        );
        assert_eq!(again, count, "the filling call reports the same count");
        assert_eq!(hits[0].page, 0);
        let snippet = unsafe { CStr::from_ptr(hits[0].snippet) };
        assert!(snippet.to_str().unwrap().contains("SteleOFD"));
    });
}

#[test]
fn search_reports_a_short_array_without_writing() {
    with_doc(false, |doc| {
        let mut errors: Errors = [0; 256];
        let needle = c"steleofd";
        let mut hit = SteleofdHit {
            page: 9,
            snippet: ptr::null(),
        };
        let count = steleofd_search(
            doc,
            needle.as_ptr(),
            &mut hit,
            0,
            errors.as_mut_ptr(),
            errors.len(),
        );
        assert_eq!(count, 1, "the count still comes back");
        assert_eq!(hit.page, 9, "a zero-length array is left alone");
        assert!(hit.snippet.is_null());

        let absent = c"absent";
        assert_eq!(
            steleofd_search(
                doc,
                absent.as_ptr(),
                ptr::null_mut(),
                0,
                errors.as_mut_ptr(),
                errors.len()
            ),
            0,
            "a needle nothing matches yields no hits"
        );
    });
}

#[test]
fn search_rejects_a_null_needle() {
    with_doc(false, |doc| {
        let mut errors: Errors = [0; 256];
        let size = steleofd_search(
            doc,
            ptr::null(),
            ptr::null_mut(),
            0,
            errors.as_mut_ptr(),
            errors.len(),
        );
        assert_eq!(size, usize::MAX);
        assert!(!message(&errors).is_empty(), "the failure explains itself");
    });
}

#[test]
fn verify_requires_a_signature_list() {
    with_doc(false, |doc| {
        let mut errors: Errors = [0; 256];
        let count = steleofd_verify(doc, ptr::null_mut(), 0, errors.as_mut_ptr(), errors.len());
        assert_eq!(count, usize::MAX);
        assert!(
            !message(&errors).is_empty(),
            "a document that declares no signatures is an error, matching the CLI"
        );
    });
}

#[test]
fn verify_reports_the_valid_signature() {
    with_doc(true, |doc| {
        let mut errors: Errors = [0; 256];
        let count = steleofd_verify(doc, ptr::null_mut(), 0, errors.as_mut_ptr(), errors.len());
        assert_eq!(count, 1);

        let mut sigs: [SteleofdSignature; 2] = std::array::from_fn(|_| SteleofdSignature {
            location: ptr::null(),
            valid: 0,
        });
        let again = steleofd_verify(
            doc,
            sigs.as_mut_ptr(),
            sigs.len(),
            errors.as_mut_ptr(),
            errors.len(),
        );
        assert_eq!(again, count);
        let location = unsafe { CStr::from_ptr(sigs[0].location) };
        assert_eq!(location.to_str().unwrap(), SIGN_FILE);
        assert_ne!(sigs[0].valid, 0, "the embedded certificate verifies");
    });
}
