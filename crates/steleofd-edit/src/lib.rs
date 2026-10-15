//! Command-pattern editing of a document model with an undo stack.
//! Edits never touch the archive directly: the caller loads a document, runs
//! commands through a session and re-serializes once the session settles.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod commands;

pub use commands::{AddObject, Command, RemoveObject, SetPageArea};

use std::sync::Arc;

use steleofd_error::{EditError, OfdError};
use steleofd_model::document::{Document, Page};

/// Which part of the document a command touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditTarget {
    /// A layer on one page.
    Page(usize),
    /// The document as a whole, for example its common data.
    Document,
}

/// Decides whether an edit may proceed.
pub trait PermissionPolicy {
    /// Whether the given target may be modified.
    fn allows(&self, target: EditTarget) -> bool;
}

/// Accepts every edit.
#[derive(Debug, Default)]
pub struct AllowAll;

impl PermissionPolicy for AllowAll {
    fn allows(&self, _target: EditTarget) -> bool {
        true
    }
}

/// Rejects edits to a named set of pages.
#[derive(Debug, Default)]
pub struct DenyPages {
    blocked: Vec<usize>,
}

impl DenyPages {
    /// Create a policy blocking the given page indices.
    pub fn new(blocked: impl IntoIterator<Item = usize>) -> Self {
        Self {
            blocked: blocked.into_iter().collect(),
        }
    }
}

impl PermissionPolicy for DenyPages {
    fn allows(&self, target: EditTarget) -> bool {
        match target {
            EditTarget::Page(index) => !self.blocked.contains(&index),
            EditTarget::Document => true,
        }
    }
}

/// Tunables of an editing session.
#[derive(Clone)]
pub struct EditOptions {
    /// Number of signatures currently protecting the document.
    pub signature_count: usize,
    /// Whether edits may proceed despite existing signatures.
    pub allow_signature_invalidation: bool,
    /// How many commands the undo stack keeps.
    pub max_history: usize,
    /// Permission policy consulted before every command.
    pub permissions: Option<Arc<dyn PermissionPolicy + Send + Sync>>,
}

impl Default for EditOptions {
    fn default() -> Self {
        Self {
            signature_count: 0,
            allow_signature_invalidation: false,
            max_history: 64,
            permissions: None,
        }
    }
}

impl std::fmt::Debug for EditOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditOptions")
            .field("signature_count", &self.signature_count)
            .field(
                "allow_signature_invalidation",
                &self.allow_signature_invalidation,
            )
            .field("max_history", &self.max_history)
            .field("has_permissions", &self.permissions.is_some())
            .finish()
    }
}

/// An editing session over one document.
#[derive(Debug)]
pub struct EditSession {
    document: Document,
    history: Vec<Box<dyn Command>>,
    cursor: usize,
    options: EditOptions,
}

impl EditSession {
    /// Wrap a document in a fresh session.
    pub fn new(document: Document, options: EditOptions) -> Self {
        Self {
            document,
            history: Vec::new(),
            cursor: 0,
            options,
        }
    }

    /// Borrow the document being edited.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Whether an undo is available.
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// Whether a redo is available.
    pub fn can_redo(&self) -> bool {
        self.cursor < self.history.len()
    }

    /// Number of retained commands.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Run one command, dropping any redo branch.
    pub fn apply(&mut self, mut command: impl Command + 'static) -> Result<(), OfdError> {
        self.check_signatures()?;
        for target in command.targets() {
            if let Some(policy) = &self.options.permissions
                && !policy.allows(target)
            {
                return Err(OfdError::Edit(EditError::PermissionDenied(format!(
                    "{target:?}"
                ))));
            }
        }
        command.apply(&mut self.document)?;
        self.history.truncate(self.cursor);
        self.history.push(Box::new(command));
        self.cursor += 1;
        if self.history.len() > self.options.max_history.max(1) {
            let drop = self.history.len() - self.options.max_history.max(1);
            self.history.drain(..drop);
            self.cursor -= drop;
        }
        Ok(())
    }

    /// Roll back the most recent command.
    pub fn undo(&mut self) -> Result<(), OfdError> {
        if self.cursor == 0 {
            return Err(OfdError::Edit(EditError::NothingToUndo));
        }
        self.cursor -= 1;
        self.history[self.cursor].undo(&mut self.document)?;
        Ok(())
    }

    /// Re-apply the command most recently undone.
    pub fn redo(&mut self) -> Result<(), OfdError> {
        if self.cursor >= self.history.len() {
            return Err(OfdError::Edit(EditError::NothingToUndo));
        }
        self.history[self.cursor].apply(&mut self.document)?;
        self.cursor += 1;
        Ok(())
    }

    fn check_signatures(&self) -> Result<(), OfdError> {
        if self.options.signature_count > 0 && !self.options.allow_signature_invalidation {
            return Err(OfdError::Edit(EditError::WouldInvalidateSignature {
                count: self.options.signature_count,
            }));
        }
        Ok(())
    }
}

/// Resolve a page by index, reporting a missing target.
pub(crate) fn page_mut(doc: &mut Document, index: usize) -> Result<&mut Page, OfdError> {
    doc.pages
        .pages
        .get_mut(index)
        .ok_or_else(|| OfdError::Edit(EditError::TargetNotFound(format!("page {index}"))))
}
