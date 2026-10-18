//! Property tests for the XML reader: no byte sequence makes the cursor panic,
//! because malformed markup is answered with an `XmlError` rather than a crash.

use proptest::prelude::*;
use steleofd_xml::Cursor;

/// Walk the cursor to the end, collecting how many events it yielded. Returns
/// `None` only if walking panicked, which the caller turns into a failure.
fn drain(xml: &str) -> Option<usize> {
    std::panic::catch_unwind(|| {
        let mut cursor = Cursor::new(xml);
        let mut seen = 0usize;
        // A hard ceiling so a bug that loops forever shows up as a failure
        // rather than hanging the test run.
        for _ in 0..100_000 {
            match cursor.next_event() {
                Ok(steleofd_xml::cursor::Event::Eof) => return seen,
                Ok(_) => seen += 1,
                Err(_) => return seen,
            }
        }
        panic!("cursor did not reach EOF within the ceiling");
    })
    .ok()
}

/// Build a well-formed document around a fragment, so the generator produces
/// documents the reader is expected to accept rather than random noise.
fn document(fragment: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Root>{fragment}</Root>")
}

fn tag_name() -> impl Strategy<Value = String> {
    "[A-Za-z][A-Za-z0-9]{0,10}"
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// Arbitrary text must never panic the cursor. This is the invariant that
    /// keeps a malformed document from taking the process down.
    #[test]
    fn arbitrary_text_never_panics(text in "\\PC{0,512}") {
        prop_assert!(drain(&text).is_some(), "cursor panicked on {text:?}");
    }

    /// Arbitrary bytes decoded lossily must never panic the cursor either, so
    /// invalid UTF-8 in an entry cannot crash the reader.
    #[test]
    fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let text = String::from_utf8_lossy(&bytes);
        prop_assert!(drain(&text).is_some(), "cursor panicked on {} bytes", bytes.len());
    }

    /// A well-formed document walks to the end without the depth counter
    /// drifting: every generated shape must parse cleanly.
    #[test]
    fn a_well_formed_document_is_walked_to_the_end(
        name in tag_name(),
        depth in 0usize..6,
    ) {
        let mut fragment = String::from("<Leaf/>");
        for i in 0..depth {
            fragment = format!("<N{i}>{fragment}</N{i}>");
        }
        let xml = document(&fragment);
        let seen = drain(&xml).expect("must not panic");
        // Root start, plus each nested start and end, plus the leaf, plus root end.
        prop_assert_eq!(seen, 1 + depth * 2 + 1 + 1, "for name {}", name);
    }

    /// An unclosed element must be reported rather than silently accepted, so a
    /// truncated document cannot be mistaken for a complete one.
    #[test]
    fn an_unclosed_element_is_reported(name in tag_name()) {
        let xml = format!("<Root><{name}>");
        let mut cursor = Cursor::new(&xml);
        let mut saw_error = false;
        for _ in 0..64 {
            match cursor.next_event() {
                Ok(steleofd_xml::cursor::Event::Eof) => break,
                Ok(_) => {}
                Err(_) => { saw_error = true; break; }
            }
        }
        prop_assert!(saw_error, "<{name}> left open must be an error");
    }

    /// Text content round-trips with only its ends trimmed: a blank inside is a
    /// character of the value, because a `TextCode` opens with one often enough
    /// and the offsets beside it count characters.
    #[test]
    fn text_content_round_trips(text in "[A-Za-z0-9.,:;_\\-][A-Za-z0-9.,:;_\\- ]{0,39}") {
        let xml = format!("<Root><Leaf>{text}</Leaf></Root>");
        let mut cursor = Cursor::new(&xml);
        cursor.expect_start("Root").expect("root");
        let leaf = cursor.expect_start("Leaf").expect("leaf");
        let _ = leaf;
        let found = cursor.read_text().expect("text");
        prop_assert_eq!(found.as_str(), text.trim());
    }

    /// A whitespace-only run is never surfaced as document text, so layout
    /// whitespace between elements cannot leak into a value.
    #[test]
    fn whitespace_only_text_is_skipped(spaces in "[ \t\r\n]{1,20}") {
        let xml = format!("<Root><Leaf/>{spaces}<Other/></Root>");
        let mut cursor = Cursor::new(&xml);
        for _ in 0..16 {
            match cursor.next_event() {
                Ok(steleofd_xml::cursor::Event::Text(t)) => {
                    prop_assert!(false, "whitespace {t:?} was surfaced");
                }
                Ok(steleofd_xml::cursor::Event::Eof) => break,
                Ok(_) => {}
                Err(e) => panic!("unexpected error for {xml:?}: {e}"),
            }
        }
    }
}
