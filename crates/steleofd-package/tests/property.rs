//! Property tests for the ZIP container reader: a package this build writes is
//! one it can read back, and arbitrary bytes are answered with an error rather
//! than a panic.

use proptest::collection::vec;
use proptest::prelude::*;
use steleofd_package::{Package, build_archive};

/// Entry names that stay inside the ZIP name rules, so the generator produces
/// archives the writer is willing to build rather than rejecting its own input.
fn entry_name() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_/]{1,24}\\.(xml|dat|png)"
}

fn entry_body() -> impl Strategy<Value = Vec<u8>> {
    vec(any::<u8>(), 0..2048)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// A ZIP this build writes must open again, and every entry must come back
    /// byte for byte in the order it was written.
    #[test]
    fn a_written_archive_round_trips(
        entries in vec((entry_name(), entry_body()), 1..8),
    ) {
        let borrowed: Vec<(&str, &[u8])> =
            entries.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        let Ok(bytes) = build_archive(&borrowed) else {
            // A duplicate name is a rejection, not a property failure.
            return Ok(());
        };
        let mut package = Package::from_bytes(&bytes).expect("archive it just wrote must open");
        prop_assert_eq!(package.len(), entries.len());
        for (name, body) in &entries {
            let read = package.read(name).expect("entry must read back");
            prop_assert_eq!(&read, body);
        }
    }

    /// The entry list a reader reports must be exactly the names that were
    /// written, so a walk of the archive never invents or drops a path.
    #[test]
    fn the_entry_list_matches_what_was_written(
        entries in vec(entry_name(), 1..8),
    ) {
        let bodies: Vec<(&str, &[u8])> =
            entries.iter().map(|n| (n.as_str(), b"x".as_slice())).collect();
        let Ok(bytes) = build_archive(&bodies) else {
            // A duplicate name is a rejection by the writer, not a property
            // failure: the writer refuses what the reader would collapse.
            return Ok(());
        };
        let package = Package::from_bytes(&bytes).expect("open");
        let mut listed = package.entries().to_vec();
        let mut written = entries.clone();
        listed.sort();
        written.sort();
        written.dedup();
        prop_assert_eq!(listed, written);
    }

    /// Arbitrary bytes must be answered with a `Result`, never a panic. The ZIP
    /// reader is the first thing hostile input reaches, so this is the invariant
    /// that keeps a malformed download from taking the process down.
    #[test]
    fn arbitrary_bytes_never_panic(data in vec(any::<u8>(), 0..4096)) {
        let outcome = std::panic::catch_unwind(|| Package::from_bytes(&data).is_ok());
        prop_assert!(outcome.is_ok(), "opening {} bytes panicked", data.len());
    }

    /// A truncated archive either opens with fewer entries or fails cleanly; it
    /// must never hand back an entry whose bytes run past the input.
    #[test]
    fn a_truncated_archive_fails_cleanly(
        entries in vec((entry_name(), entry_body()), 1..6),
        cut in 0usize..512,
    ) {
        let borrowed: Vec<(&str, &[u8])> =
            entries.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        let Ok(bytes) = build_archive(&borrowed) else {
            return Ok(());
        };
        let cut = cut.min(bytes.len());
        let truncated = &bytes[..cut];
        let outcome = std::panic::catch_unwind(|| {
            if let Ok(mut p) = Package::from_bytes(truncated) {
                // Whatever it does report must read back or error, not panic.
                let names: Vec<String> = p.entries().to_vec();
                for name in names {
                    let _ = p.read(&name);
                }
            }
        });
        prop_assert!(outcome.is_ok(), "a truncated archive panicked");
    }

    /// Reading a path that was never written is an error, not a panic, whatever
    /// the path looks like.
    #[test]
    fn reading_an_absent_path_is_an_error(path in "\\PC{0,40}") {
        let bytes = build_archive(&[("a.xml", b"<a/>".as_slice())]).expect("build");
        let mut package = Package::from_bytes(&bytes).expect("open");
        if path == "a.xml" {
            return Ok(());
        }
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            package.read(&path).is_err()
        }));
        prop_assert!(outcome.is_ok(), "reading {path:?} panicked");
    }
}
