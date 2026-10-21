//! The threading contract the header states, exercised: every shape it allows,
//! since what it forbids is undefined behaviour no signature can refuse.

use std::ffi::{CStr, c_char};
use std::ptr;

use steleofd_ffi::*;
use steleofd_testkit::fixture_bytes;

/// A scratch error buffer standing in for the caller's, sized as C would.
type Errors = [c_char; 256];

/// Open the unsigned fixture and release it whatever the body does.
fn with_doc<R>(body: impl FnOnce(*mut SteleofdDocument) -> R) -> R {
    let bytes = fixture_bytes(false);
    let mut errors: Errors = [0; 256];
    let doc = steleofd_open(
        bytes.as_ptr(),
        bytes.len(),
        errors.as_mut_ptr(),
        errors.len(),
    );
    assert!(!doc.is_null(), "the fixture opens");
    let result = body(doc);
    steleofd_close(doc);
    result
}

/// The PNG size at 96 dpi, through the probing form of the call.
fn png_size(doc: *mut SteleofdDocument, errors: &mut Errors) -> usize {
    steleofd_render_png(
        doc,
        0,
        96.0,
        ptr::null_mut(),
        0,
        errors.as_mut_ptr(),
        errors.len(),
    )
}

/// The document text a search finds, read back through the borrowing rule.
fn first_snippet(doc: *mut SteleofdDocument, errors: &mut Errors) -> String {
    let mut hit = SteleofdHit {
        page: 0,
        snippet: ptr::null(),
    };
    let count = steleofd_search(
        doc,
        c"steleofd".as_ptr(),
        &mut hit,
        1,
        errors.as_mut_ptr(),
        errors.len(),
    );
    assert_eq!(count, 1, "the page carries the text once");
    // The rule the header states: the pointer survives until the next call on
    // this handle that hands out borrowed data, so it is read now.
    unsafe { CStr::from_ptr(hit.snippet) }
        .to_string_lossy()
        .into_owned()
}

#[test]
fn a_handle_opened_on_one_thread_works_on_another() {
    // What the original host does: load the library, open on a worker, and
    // never touch the handle from the thread that spawned it. Nothing in the
    // document points back at the opener.
    let bytes = fixture_bytes(false);
    let worker = std::thread::spawn(move || {
        let mut errors: Errors = [0; 256];
        let doc = steleofd_open(
            bytes.as_ptr(),
            bytes.len(),
            errors.as_mut_ptr(),
            errors.len(),
        );
        assert!(!doc.is_null(), "the fixture opens on this thread");
        let pages = steleofd_page_count(doc);
        let snippet = first_snippet(doc, &mut errors);
        steleofd_close(doc);
        (pages, snippet)
    });
    let (pages, snippet) = worker.join().expect("the worker finishes");
    assert_eq!(pages, 1);
    assert!(snippet.contains("SteleOFD"));
}

#[test]
fn two_handles_render_in_parallel() {
    // A server answering two uploads at once is the shape this promises: one
    // document per thread, no handle shared, no shared mutable state between
    // them.
    let sizes: Vec<usize> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    with_doc(|doc| {
                        let mut errors: Errors = [0; 256];
                        png_size(doc, &mut errors)
                    })
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("each thread finishes"))
            .collect()
    });
    assert!(
        sizes.iter().all(|size| *size > 1000),
        "every thread rendered a page: {sizes:?}"
    );
    assert!(
        sizes.windows(2).all(|pair| pair[0] == pair[1]),
        "one page renders to the same bytes whichever thread did it: {sizes:?}"
    );
}

#[test]
fn a_handle_is_sendable_so_ownership_moves_rather_than_being_shared() {
    // The handle is `Send` and `Sync`, because every field inside it is: the
    // document's font manager holds its caches behind locks. What two threads
    // may not share is one C pointer, which is what the contract is about.
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<SteleofdDocument>();
    assert_sync::<SteleofdDocument>();
}
