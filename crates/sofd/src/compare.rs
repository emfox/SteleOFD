//! Comparing two revisions of a document. The comparison runs over the display
//! list both sides already have after opening, so two files that draw the same
//! page compare equal even when their markup differs.

/// How one page of the newer revision differs from the older one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageChange {
    /// The page is present in both and draws the same content.
    Unchanged,
    /// The page was added by the newer revision.
    Added,
    /// The page was dropped by the newer revision.
    Removed,
    /// The page is present in both but its box changed size.
    Resized,
    /// The page draws the same shapes but different text.
    Text,
    /// The page's drawing changed in a way that is not text alone.
    Graphics,
}

/// One page's verdict over the two revisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageDiff {
    /// Zero-based page index.
    pub index: usize,
    /// What changed on the page.
    pub change: PageChange,
    /// The text the older revision drew on the page, in run order.
    pub before_text: String,
    /// The text the newer revision drew on the page.
    pub after_text: String,
}

/// The comparison of two revisions, page by page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// Pages the older revision holds.
    pub before_pages: usize,
    /// Pages the newer revision holds.
    pub after_pages: usize,
    /// One entry per page index either side holds, in order.
    pub pages: Vec<PageDiff>,
}

impl Comparison {
    /// Whether the two revisions draw the same thing.
    pub fn is_identical(&self) -> bool {
        self.pages
            .iter()
            .all(|page| page.change == PageChange::Unchanged)
    }

    /// How many pages changed, by any measure.
    pub fn changed_pages(&self) -> usize {
        self.pages
            .iter()
            .filter(|page| page.change != PageChange::Unchanged)
            .count()
    }
}

mod judge;

pub use judge::compare;
