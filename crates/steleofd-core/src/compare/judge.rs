//! How one page reads after a comparison: resized, retyped, redrawn, or the
//! same. The verdicts are ordered by how specific they are, which is why a
//! change in text is reported as `Text` rather than folded into `Graphics`.

use steleofd_display::{DrawOp, PageDisplay};

use super::{Comparison, PageChange, PageDiff};

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
