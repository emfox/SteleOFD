//! Comparing two revisions of a document. The comparison runs over the display
//! list both sides already have after opening, so two files that draw the same
//! page compare equal even when their markup differs.

use steleofd_display::{DrawOp, PageDisplay};

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

/// Compare two display lists page by page. A page present only in `after` is
/// [`PageChange::Added`], only in `before` is [`PageChange::Removed`], and a
/// page in both is judged by its box, its text and its remaining commands.
pub fn compare(before: &[PageDisplay], after: &[PageDisplay]) -> Comparison {
    let total = before.len().max(after.len());
    let mut pages = Vec::with_capacity(total);
    for index in 0..total {
        let old = before.get(index);
        let new = after.get(index);
        let (change, before_text, after_text) = match (old, new) {
            (Some(old), Some(new)) => judge(old, new),
            (None, Some(new)) => (PageChange::Added, String::new(), text_of(new)),
            (Some(old), None) => (PageChange::Removed, text_of(old), String::new()),
            (None, None) => (PageChange::Unchanged, String::new(), String::new()),
        };
        pages.push(PageDiff {
            index,
            change,
            before_text,
            after_text,
        });
    }
    Comparison {
        before_pages: before.len(),
        after_pages: after.len(),
        pages,
    }
}

/// Judge one page that both revisions hold.
fn judge(old: &PageDisplay, new: &PageDisplay) -> (PageChange, String, String) {
    let before_text = text_of(old);
    let after_text = text_of(new);
    if !same_box(old, new) {
        return (PageChange::Resized, before_text, after_text);
    }
    if before_text == after_text && shapes_of(old) == shapes_of(new) {
        return (PageChange::Unchanged, before_text, after_text);
    }
    // Text differing is the most specific change, so it is reported before the
    // broader Graphics verdict when the shapes are otherwise the same.
    if before_text != after_text && shapes_of(old) == shapes_of(new) {
        return (PageChange::Text, before_text, after_text);
    }
    (PageChange::Graphics, before_text, after_text)
}

/// Whether two pages share a physical box.
fn same_box(a: &PageDisplay, b: &PageDisplay) -> bool {
    (a.page_box.w - b.page_box.w).abs() < 1e-6 && (a.page_box.h - b.page_box.h).abs() < 1e-6
}

/// The text a page draws, run by run, non-text commands dropped.
fn text_of(page: &PageDisplay) -> String {
    page.ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::Text(run) => Some(run),
            _ => None,
        })
        .filter_map(|run| run.text.clone())
        .collect::<Vec<_>>()
        .join("")
}

/// A fingerprint of a page's non-text drawing, so a shape change is visible
/// without comparing every field of every command.
fn shapes_of(page: &PageDisplay) -> Vec<String> {
    page.ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::Text(_) => None,
            DrawOp::Path { path, .. } => Some(format!("path:{}", path.len())),
            DrawOp::Image { dst, .. } => Some(format!("image:{:.3}x{:.3}", dst.w, dst.h)),
            DrawOp::ClipPath { path, .. } => Some(format!("clip:{}", path.len())),
            DrawOp::ClipText(_) => Some("cliptext".to_string()),
            DrawOp::Save => None,
            DrawOp::Restore => None,
            DrawOp::Transform(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use steleofd_common::{Millimeter, Rect};
    use steleofd_display::GlyphRun;
    use steleofd_display::ops::{Color, Paint};
    use steleofd_font::FontId;
    use steleofd_geometry::{Affine, Path};

    /// A page holding one text run with the given text.
    fn page(index: usize, text: &str) -> PageDisplay {
        PageDisplay {
            index,
            page_box: Rect::A4_PORTRAIT,
            ops: vec![DrawOp::Text(run(text, Rect::new(10.0, 20.0, 30.0, 4.0)))],
            content_bbox: Rect::new(10.0, 20.0, 30.0, 4.0),
        }
    }

    /// A page holding one filled square, no text.
    fn shape_page(index: usize) -> PageDisplay {
        let path = Path::build(|b| {
            b.move_to(0.0, 0.0)
                .line_to(10.0, 0.0)
                .line_to(10.0, 10.0)
                .close();
        });
        PageDisplay {
            index,
            page_box: Rect::A4_PORTRAIT,
            ops: vec![DrawOp::Path {
                path,
                fill: Some(Paint::Solid(Color::BLACK)),
                stroke: None,
                stroke_style: steleofd_display::StrokeStyle::default(),
                rule: steleofd_display::PaintRule::NonZero,
            }],
            content_bbox: Rect::new(0.0, 0.0, 10.0, 10.0),
        }
    }

    fn run(text: &str, bbox: Rect) -> GlyphRun {
        GlyphRun {
            font: FontId(0),
            size: Millimeter(3.5),
            glyphs: vec![1, 2, 3],
            advances: vec![1.0, 1.0, 1.0],
            text: Some(text.to_string()),
            cluster_map: Some(vec![0, 1, 2]),
            codes_are_glyphs: false,
            transform: Affine::IDENTITY,
            glyph_angle: 0.0,
            fill: Some(Paint::Solid(Color::BLACK)),
            stroke: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            bbox,
        }
    }

    #[test]
    fn identical_pages_are_unchanged() {
        let before = vec![page(0, "hello")];
        let after = vec![page(0, "hello")];
        let diff = compare(&before, &after);
        assert!(diff.is_identical());
        assert_eq!(diff.changed_pages(), 0);
    }

    #[test]
    fn a_text_edit_reads_as_a_text_change() {
        let diff = compare(&[page(0, "hello")], &[page(0, "world")]);
        assert_eq!(diff.pages[0].change, PageChange::Text);
        assert_eq!(diff.pages[0].before_text, "hello");
        assert_eq!(diff.pages[0].after_text, "world");
    }

    #[test]
    fn a_shape_edit_reads_as_a_graphics_change() {
        let mut after = shape_page(0);
        after.content_bbox = Rect::new(0.0, 0.0, 20.0, 20.0);
        after.ops.push(DrawOp::Path {
            path: Path::build(|b| {
                b.move_to(5.0, 5.0).line_to(15.0, 15.0).close();
            }),
            fill: Some(Paint::Solid(Color::BLACK)),
            stroke: None,
            stroke_style: steleofd_display::StrokeStyle::default(),
            rule: steleofd_display::PaintRule::NonZero,
        });
        let diff = compare(&[shape_page(0)], &[after]);
        assert_eq!(diff.pages[0].change, PageChange::Graphics);
    }

    #[test]
    fn an_added_and_a_removed_page_are_named() {
        let before = vec![page(0, "one")];
        let after = vec![page(0, "one"), page(1, "two")];
        let diff = compare(&before, &after);
        assert_eq!(diff.before_pages, 1);
        assert_eq!(diff.after_pages, 2);
        assert_eq!(diff.pages[0].change, PageChange::Unchanged);
        assert_eq!(diff.pages[1].change, PageChange::Added);
        assert_eq!(diff.pages[1].after_text, "two");

        let reversed = compare(&after, &before);
        assert_eq!(reversed.pages[1].change, PageChange::Removed);
        assert_eq!(reversed.pages[1].before_text, "two");
    }

    #[test]
    fn a_resized_page_reads_as_resized() {
        let mut after = page(0, "hello");
        after.page_box = Rect::new(0.0, 0.0, 100.0, 200.0);
        let diff = compare(&[page(0, "hello")], &[after]);
        assert_eq!(diff.pages[0].change, PageChange::Resized);
    }
}
