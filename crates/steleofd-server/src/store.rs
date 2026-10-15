//! The documents the service holds open, and the lock each one carries.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use steleofd_core::Document;
use uuid::Uuid;

/// One open document, shared by every request working on it.
pub type Shared = Arc<Mutex<Document>>;

/// The open documents, keyed by the id the API hands out.
#[derive(Clone)]
pub struct Store {
    open: Arc<Mutex<HashMap<Uuid, Shared>>>,
    capacity: usize,
}

impl Store {
    /// An empty store that will hold at most `capacity` documents.
    pub fn new(capacity: usize) -> Self {
        Self {
            open: Arc::new(Mutex::new(HashMap::new())),
            capacity,
        }
    }

    /// Keep a document under a fresh id and hand the id back.
    pub fn insert(&self, doc: Document) -> Uuid {
        let id = Uuid::new_v4();
        self.lock().insert(id, Arc::new(Mutex::new(doc)));
        id
    }

    /// The document behind an id, if it is still open.
    pub fn get(&self, id: Uuid) -> Option<Shared> {
        self.lock().get(&id).cloned()
    }

    /// Drop a document, reporting whether it was still open.
    pub fn remove(&self, id: Uuid) -> bool {
        self.lock().remove(&id).is_some()
    }

    /// How many documents are open.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether no document is open.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the store is at its capacity, so opening has to be refused.
    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity
    }

    /// The poisoned-lock fallback: a panic mid-insert leaves the map usable,
    /// and refusing every later request over it would be worse.
    fn lock(&self) -> MutexGuard<'_, HashMap<Uuid, Shared>> {
        self.open.lock().unwrap_or_else(|err| err.into_inner())
    }
}

/// Lock a document for work, the same way [`Store`] locks its map.
pub fn lock(doc: &Shared) -> MutexGuard<'_, Document> {
    doc.lock().unwrap_or_else(|err| err.into_inner())
}
