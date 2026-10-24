//! Every string-level reader returns `Ok` or `Err`, never panicking and never
//! hanging on any input. Weaker than correctness, but it is the property a
//! reader of untrusted files actually depends on.

use proptest::prelude::*;
use sofd_parser::content;

/// Run one reader over one input and report whether it panicked.
fn no_panic(label: &str, input: &str, run: impl FnOnce(&str) + std::panic::UnwindSafe) {
    // The panic hook would otherwise print a backtrace for every generated
    // case, drowning the one line that matters.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = std::panic::catch_unwind(move || run(input));
    std::panic::set_hook(previous);
    assert!(outcome.is_ok(), "{label} panicked on {input:?}");
}

/// Feed one string to every string-level reader the parser exposes.
fn exercise(input: &str) {
    let _ = content::read_ofd_root(input);
    let _ = content::read_document(input);
    let _ = content::read_page_file(input);
    let _ = content::read_template_block(input);
    let _ = content::read_multimedias(input);
    let _ = content::read_composite_graphic_units(input);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    /// Arbitrary text reaches every reader without a panic.
    #[test]
    fn arbitrary_text_never_panics_a_reader(text in "\\PC{0,600}") {
        no_panic("the parser readers", &text, exercise);
    }

    /// Arbitrary bytes decoded lossily reach every reader without a panic, so
    /// invalid UTF-8 in an entry cannot crash the load path.
    #[test]
    fn arbitrary_bytes_never_panic_a_reader(
        bytes in proptest::collection::vec(any::<u8>(), 0..1024),
    ) {
        let text = String::from_utf8_lossy(&bytes).into_owned();
        no_panic("the parser readers", &text, exercise);
    }

    /// Well-formed markup with a hostile payload inside still returns cleanly:
    /// a document whose root is right but whose contents are nonsense is the
    /// common real-world case and must not crash.
    #[test]
    fn hostile_content_under_a_known_root_never_panics(payload in "\\PC{0,400}") {
        let wrapped = format!("<?xml version=\"1.0\"?><Document>{payload}</Document>");
        no_panic("read_document", &wrapped, |s| { let _ = content::read_document(s); });

        let wrapped = format!("<?xml version=\"1.0\"?><OFD>{payload}</OFD>");
        no_panic("read_ofd_root", &wrapped, |s| { let _ = content::read_ofd_root(s); });

        let wrapped = format!("<?xml version=\"1.0\"?><Page>{payload}</Page>");
        no_panic("read_page_file", &wrapped, |s| { let _ = content::read_page_file(s); });
    }

    /// A truncated well-formed document is an error, not a crash, whatever
    /// point it is cut at.
    #[test]
    fn a_truncated_document_never_panics(cut in 0usize..200) {
        let full = r#"<?xml version="1.0"?><Document><Pages><Page ID="1"/></Pages></Document>"#;
        let cut = cut.min(full.len());
        let truncated = &full[..cut];
        no_panic("read_document", truncated, |s| { let _ = content::read_document(s); });
    }
}
