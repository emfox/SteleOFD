//! Concrete edit commands over the document model.

use crate::{EditTarget, page_mut};
use sofd_error::{EditError, OfdError};
use sofd_model::document::{Document, PageArea};
use sofd_model::object::Object;

/// One reversible change to a document.
pub trait Command: std::fmt::Debug {
    /// Parts of the document this command touches, used by permission checks.
    fn targets(&self) -> Vec<EditTarget>;
    /// Human readable summary, for history displays.
    fn description(&self) -> &str;
    /// Perform the change.
    fn apply(&mut self, document: &mut Document) -> Result<(), OfdError>;
    /// Restore the previous state.
    fn undo(&mut self, document: &mut Document) -> Result<(), OfdError>;
}

/// Append an object at the end of a page layer.
#[derive(Debug, Clone)]
pub struct AddObject {
    /// Page to modify.
    pub page_index: usize,
    /// Layer within that page.
    pub layer_index: usize,
    /// Object to insert.
    pub object: Object,
    /// Index the object landed on, filled in on apply.
    pub inserted_at: Option<usize>,
}

impl Command for AddObject {
    fn targets(&self) -> Vec<EditTarget> {
        vec![EditTarget::Page(self.page_index)]
    }

    fn description(&self) -> &str {
        "add object"
    }

    fn apply(&mut self, document: &mut Document) -> Result<(), OfdError> {
        let page = page_mut(document, self.page_index)?;
        let content = page.content.get_or_insert_with(Default::default);
        let layer = content.layers.get_mut(self.layer_index).ok_or_else(|| {
            OfdError::Edit(EditError::TargetNotFound(format!(
                "layer {} on page {}",
                self.layer_index, self.page_index
            )))
        })?;
        layer.objects.push(self.object.clone());
        self.inserted_at = Some(layer.objects.len() - 1);
        Ok(())
    }

    fn undo(&mut self, document: &mut Document) -> Result<(), OfdError> {
        let page = page_mut(document, self.page_index)?;
        let content = page
            .content
            .as_mut()
            .ok_or_else(|| OfdError::Edit(EditError::TargetNotFound("page content".into())))?;
        let index = self.inserted_at.ok_or_else(|| {
            OfdError::Edit(EditError::TargetNotFound("object never inserted".into()))
        })?;
        let layer = content.layers.get_mut(self.layer_index).ok_or_else(|| {
            OfdError::Edit(EditError::TargetNotFound(format!(
                "layer {} on page {}",
                self.layer_index, self.page_index
            )))
        })?;
        if index >= layer.objects.len() {
            return Err(OfdError::Edit(EditError::TargetNotFound(
                "object already removed".into(),
            )));
        }
        layer.objects.remove(index);
        Ok(())
    }
}

/// Remove one object from a page layer.
#[derive(Debug, Clone)]
pub struct RemoveObject {
    /// Page to modify.
    pub page_index: usize,
    /// Layer within that page.
    pub layer_index: usize,
    /// Index of the object inside that layer.
    pub object_index: usize,
    /// Removed object, kept for undo.
    pub removed: Option<Object>,
}

impl Command for RemoveObject {
    fn targets(&self) -> Vec<EditTarget> {
        vec![EditTarget::Page(self.page_index)]
    }

    fn description(&self) -> &str {
        "remove object"
    }

    fn apply(&mut self, document: &mut Document) -> Result<(), OfdError> {
        let page = page_mut(document, self.page_index)?;
        let content = page
            .content
            .as_mut()
            .ok_or_else(|| OfdError::Edit(EditError::TargetNotFound("page content".into())))?;
        let layer = content.layers.get_mut(self.layer_index).ok_or_else(|| {
            OfdError::Edit(EditError::TargetNotFound(format!(
                "layer {} on page {}",
                self.layer_index, self.page_index
            )))
        })?;
        if self.object_index >= layer.objects.len() {
            return Err(OfdError::Edit(EditError::TargetNotFound(format!(
                "object {} in layer {}",
                self.object_index, self.layer_index
            ))));
        }
        self.removed = Some(layer.objects.remove(self.object_index));
        Ok(())
    }

    fn undo(&mut self, document: &mut Document) -> Result<(), OfdError> {
        let object = self.removed.take().ok_or_else(|| {
            OfdError::Edit(EditError::TargetNotFound("object never removed".into()))
        })?;
        let page = page_mut(document, self.page_index)?;
        let content = page
            .content
            .as_mut()
            .ok_or_else(|| OfdError::Edit(EditError::TargetNotFound("page content".into())))?;
        let layer = content.layers.get_mut(self.layer_index).ok_or_else(|| {
            OfdError::Edit(EditError::TargetNotFound(format!(
                "layer {} on page {}",
                self.layer_index, self.page_index
            )))
        })?;
        let index = self.object_index.min(layer.objects.len());
        layer.objects.insert(index, object);
        Ok(())
    }
}

/// Replace one page's extent.
#[derive(Debug, Clone)]
pub struct SetPageArea {
    /// Page to modify.
    pub page_index: usize,
    /// New extent.
    pub area: PageArea,
    /// Previous extent, kept for undo.
    pub previous: Option<PageArea>,
}

impl Command for SetPageArea {
    fn targets(&self) -> Vec<EditTarget> {
        vec![EditTarget::Page(self.page_index)]
    }

    fn description(&self) -> &str {
        "set page area"
    }

    fn apply(&mut self, document: &mut Document) -> Result<(), OfdError> {
        let page = page_mut(document, self.page_index)?;
        self.previous = page.area.replace(self.area);
        Ok(())
    }

    fn undo(&mut self, document: &mut Document) -> Result<(), OfdError> {
        let page = page_mut(document, self.page_index)?;
        match self.previous.take() {
            Some(area) => {
                page.area = Some(area);
                Ok(())
            }
            None => {
                page.area = None;
                Ok(())
            }
        }
    }
}
