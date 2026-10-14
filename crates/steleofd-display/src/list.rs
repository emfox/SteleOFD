//! Display lists and their builder.

use serde::{Deserialize, Serialize};
use steleofd_common::Rect;
use steleofd_geometry::Affine;

use crate::ops::DrawOp;

/// The flattened display content of one page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageDisplay {
    /// Zero-based page index.
    pub index: usize,
    /// Physical page size in millimetres.
    pub page_box: Rect,
    /// Flattened drawing commands.
    pub ops: Vec<DrawOp>,
    /// Union of all drawn geometry, used to detect blank pages and cull.
    pub content_bbox: Rect,
}

impl PageDisplay {
    /// Whether the page has nothing visible to draw.
    pub fn is_blank(&self) -> bool {
        self.ops.is_empty() || self.content_bbox.area() <= 0.0
    }

    /// Cheap upper bound on commands that may survive viewport culling.
    pub fn visible_ops_count(&self, viewport: Rect) -> usize {
        if !self.content_bbox.intersects(viewport) {
            return 0;
        }
        self.ops.len()
    }
}

/// Display list for a whole document, produced page by page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DisplayList {
    /// Total pages in the document.
    pub page_count: usize,
    /// Pages materialized so far; large documents may hold only a window.
    pub pages: Vec<PageDisplay>,
}

impl DisplayList {
    /// Look up a page by index.
    pub fn page(&self, index: usize) -> Option<&PageDisplay> {
        self.pages.iter().find(|p| p.index == index)
    }
}

/// Lazy handle to a page.
/// Large documents need not be fully flattened; a backend resolves a page only
/// when it needs it, which makes shard loading natural.
pub struct PageRef<'a> {
    /// Page index this handle resolves to.
    pub index: usize,
    /// Deferred producer of the page display list.
    pub resolve: Box<dyn FnOnce() -> steleofd_error::Result<PageDisplay> + 'a>,
}

/// Builds a [`PageDisplay`] by feeding flattened commands.
/// Owns the graphics state stack, so emitted commands already carry absolute
/// coordinates and backends never re-apply the CTM.
#[derive(Debug)]
pub struct DisplayListBuilder {
    page_box: Rect,
    ops: Vec<DrawOp>,
    state_stack: Vec<Affine>,
    current: Affine,
    content_bbox: Rect,
    max_depth: usize,
}

impl DisplayListBuilder {
    /// Create a builder for a page of the given size.
    pub fn new(page_box: Rect) -> Self {
        Self {
            page_box,
            ops: Vec::with_capacity(64),
            state_stack: Vec::with_capacity(8),
            current: Affine::IDENTITY,
            content_bbox: Rect::default(),
            max_depth: 64,
        }
    }

    /// Cap the graphics state stack, guarding against hostile nesting depth.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// The transform currently in effect.
    pub fn current_transform(&self) -> Affine {
        self.current
    }

    /// Push the graphics state.
    pub fn save(&mut self) -> steleofd_error::Result<()> {
        if self.state_stack.len() >= self.max_depth {
            return Err(steleofd_error::FormatError::MaxDepthExceeded(self.max_depth).into());
        }
        self.state_stack.push(self.current);
        self.ops.push(DrawOp::Save);
        Ok(())
    }

    /// Pop the graphics state.
    pub fn restore(&mut self) -> steleofd_error::Result<()> {
        let prev = self.state_stack.pop().ok_or_else(|| {
            steleofd_error::OfdError::Format(steleofd_error::FormatError::CircularReference {
                chain: "restore without matching save".into(),
            })
        })?;
        self.current = prev;
        self.ops.push(DrawOp::Restore);
        Ok(())
    }

    /// Concatenate a transform onto the current state.
    pub fn transform(&mut self, t: Affine) {
        self.current = self.current.then(t);
        self.ops.push(DrawOp::Transform(t));
    }

    /// Append a command and fold its extent into the content bounds.
    pub fn push(&mut self, op: DrawOp) {
        if let Some(b) = op_bbox(&op) {
            self.content_bbox = self.content_bbox.union(b);
        }
        self.ops.push(op);
    }

    /// Finish the page.
    pub fn finish(self, index: usize) -> PageDisplay {
        PageDisplay {
            index,
            page_box: self.page_box,
            ops: self.ops,
            content_bbox: self.content_bbox,
        }
    }
}

/// Conservative extent of a command, used only for bounds and blank detection.
fn op_bbox(op: &DrawOp) -> Option<Rect> {
    match op {
        DrawOp::Path { path, .. } => Some(path.bounding_box()),
        DrawOp::Text(run) => Some(run.bbox),
        DrawOp::Image { dst, .. } => Some(*dst),
        DrawOp::LinearGradient { path, .. } | DrawOp::RadialGradient { path, .. } => {
            Some(path.bounding_box())
        }
        DrawOp::ClipPath { path, .. } => Some(path.bounding_box()),
        _ => None,
    }
}
