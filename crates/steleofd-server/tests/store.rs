//! The document store: memory-only by default, on disk when asked, and the two
//! behave the same from the API's point of view.

use steleofd_core::Document;
use steleofd_server::Store;
use steleofd_testkit::fixture_bytes;
use uuid::Uuid;

/// A store backed by a fresh temporary directory, and that directory.
fn persistent(capacity: usize) -> (Store, tempdir::Temp) {
    let dir = tempdir::Temp::new("steleofd-store");
    let store = Store::persistent(capacity, dir.path()).expect("a store");
    (store, dir)
}

#[test]
fn a_memory_store_says_so_and_keeps_no_files() {
    let store = Store::new(4);
    assert!(!store.is_persistent());
    assert_eq!(store.root(), None);
}

#[test]
fn a_persistent_store_creates_its_directory() {
    let dir = tempdir::Temp::new("steleofd-store");
    let nested = dir.path().join("documents");
    assert!(!nested.exists(), "the directory starts absent");
    let store = Store::persistent(4, &nested).expect("a store");
    assert!(store.is_persistent());
    assert_eq!(store.root(), Some(nested.as_path()));
    assert!(nested.is_dir(), "the store made the directory it was given");
}

#[test]
fn an_inserted_document_is_readable_and_written_to_disk() {
    let (store, dir) = persistent(4);
    let doc = Document::open_bytes(&fixture_bytes(false)).expect("open");
    let id = store.insert(doc, &fixture_bytes(false)).expect("insert");

    assert_eq!(store.len(), 1);
    assert!(store.get(id).expect("read").is_some(), "the id resolves");

    let files: Vec<String> = std::fs::read_dir(dir.path())
        .expect("read the directory")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        files,
        vec![format!("{id}.ofd")],
        "one file, named for the id"
    );
}

#[test]
fn a_document_survives_a_restart() {
    // The whole point of persistence: a second store over the same directory
    // adopts what the first wrote, so an id handed out before the restart is
    // still usable after it without the caller resending anything.
    let dir = tempdir::Temp::new("steleofd-store");
    let id = {
        let store = Store::persistent(4, dir.path()).expect("a store");
        let doc = Document::open_bytes(&fixture_bytes(false)).expect("open");
        store.insert(doc, &fixture_bytes(false)).expect("insert")
    };

    let restarted = Store::persistent(4, dir.path()).expect("a second store");
    assert_eq!(restarted.len(), 1, "the document was adopted");
    let doc = restarted
        .get(id)
        .expect("read")
        .expect("the id survived the restart");
    assert_eq!(
        steleofd_server::store::lock(&doc).page_count(),
        1,
        "and it opens again into a usable document"
    );
}

#[test]
fn a_removed_document_loses_its_file_too() {
    let (store, dir) = persistent(4);
    let doc = Document::open_bytes(&fixture_bytes(false)).expect("open");
    let id = store.insert(doc, &fixture_bytes(false)).expect("insert");
    assert!(dir.path().join(format!("{id}.ofd")).exists());

    assert!(store.remove(id), "the id was open");
    assert!(!store.remove(id), "and now it is not");
    assert!(
        !dir.path().join(format!("{id}.ofd")).exists(),
        "the file went with it, so a restart does not resurrect it"
    );
    assert!(store.is_empty());
}

#[test]
fn a_memory_store_forgets_everything_on_restart() {
    // The control for the persistence test: with no directory there is
    // nothing to adopt, which is what a test process wants.
    let store = Store::new(4);
    let doc = Document::open_bytes(&fixture_bytes(false)).expect("open");
    let id = store.insert(doc, &fixture_bytes(false)).expect("insert");
    assert!(store.get(id).expect("read").is_some());

    let restarted = Store::new(4);
    assert!(restarted.is_empty());
    assert!(restarted.get(id).expect("read").is_none());
}

#[test]
fn capacity_counts_both_stores_the_same_way() {
    let (store, _dir) = persistent(1);
    let doc = Document::open_bytes(&fixture_bytes(false)).expect("open");
    store.insert(doc, &fixture_bytes(false)).expect("insert");
    assert!(store.is_full(), "one document fills a one-document store");
}

#[test]
fn an_unknown_id_is_missing_rather_than_an_error() {
    let (store, _dir) = persistent(4);
    assert!(
        store.get(Uuid::new_v4()).expect("read").is_none(),
        "a ghost id is absent, not a failure"
    );
}

/// A temporary directory that removes itself, so a test run leaves no litter.
mod tempdir {
    use std::path::{Path, PathBuf};

    /// A directory under the system temporary area, deleted on drop.
    pub struct Temp {
        path: PathBuf,
    }

    impl Temp {
        /// Make a fresh directory named after `prefix`.
        pub fn new(prefix: &str) -> Self {
            let unique = std::process::id();
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir().join(format!("{prefix}-{unique}-{nanos}"));
            std::fs::create_dir_all(&path).expect("make the directory");
            Self { path }
        }

        /// The directory.
        pub fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
