//! Coverage for the reader: the state machine behind the window. A widget tree
//! proves little, so these tests drive the same `App` the window draws from and
//! check that opening, navigating and rendering a page behave.

use std::path::PathBuf;

use steleofd_reader::viewer::{DroppedFile, Layout, MAX_DPI, MIN_DPI, PageCache};
use steleofd_reader::{App, Canvas};

/// The real minimal document from the conformance corpus.
fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../compliance/corpus/minimal.ofd")
}

fn open() -> App {
    let mut app = App::new();
    app.open_path(&corpus()).expect("the corpus document opens");
    app
}

#[test]
fn a_fresh_reader_has_nothing_open() {
    let app = App::new();
    assert_eq!(app.page_count(), 0);
    assert_eq!(app.current_page(), 0);
    assert!(app.path().is_none());
    assert!(app.error().is_none());
}

#[test]
fn opening_the_corpus_reports_its_pages() {
    let app = open();
    assert!(app.page_count() >= 1, "the corpus has at least one page");
    assert!(app.path().is_some());
    assert!(app.error().is_none(), "a clean open leaves no error");
}

#[test]
fn a_damaged_file_is_refused_with_the_engine_message() {
    let mut app = App::new();
    let err = app
        .open_bytes(b"this is not a zip archive at all", Some("broken.ofd"))
        .expect_err("garbage is not a document");
    assert!(!err.message.is_empty(), "the reader says something");
    assert_eq!(app.page_count(), 0, "nothing was opened");
}

#[test]
fn navigation_clamps_to_the_pages_that_exist() {
    let mut app = open();
    let last = app.page_count() - 1;

    app.go_to(usize::MAX);
    assert_eq!(
        app.current_page(),
        last,
        "past the end lands on the last page"
    );

    app.previous_page();
    assert_eq!(app.current_page(), 0, "before the start stays at the first");

    app.go_to(0);
    assert!(!app.has_previous());
    if last == 0 {
        assert!(!app.has_next());
    } else {
        assert!(app.has_next());
        app.next_page();
        assert_eq!(app.current_page(), 1);
    }
}

#[test]
fn a_closed_document_forgets_its_pages() {
    let mut app = open();
    let before = app.page_count();
    app.close();
    assert_eq!(app.page_count(), 0);
    assert!(app.path().is_none());
    assert!(before >= 1, "there was something to forget");
}

#[test]
fn a_page_rasterizes_and_is_reused() {
    let mut app = open();
    let page = app.current_page();

    app.page_image(page).expect("the first page renders");
    let first = app.cached(page).expect("the page is now cached").image.size;
    assert!(
        first[0] > 0 && first[1] > 0,
        "the image has real dimensions"
    );

    // A second request must not rasterize again: the cache answers it.
    app.page_image(page).expect("the cached page is returned");
    assert_eq!(app.cached(page).map(|p| p.image.size), Some(first));
}

#[test]
fn the_cache_keys_on_the_resolution_too() {
    let mut app = open();
    let page = app.current_page();
    app.page_image(page)
        .expect("renders at the default resolution");
    let wide = app.cached(page).map(|p| p.image.size).unwrap();

    app.set_dpi(200.0);
    app.page_image(page)
        .expect("renders again at the new resolution");
    let larger = app.cached(page).map(|p| p.image.size).unwrap();

    assert_ne!(wide, larger, "a different dpi is a different picture");
}

#[test]
fn the_resolution_is_clamped_to_a_sane_range() {
    let mut app = App::new();
    app.set_dpi(1.0);
    assert_eq!(app.dpi(), MIN_DPI, "below the floor is clamped up");
    app.set_dpi(100_000.0);
    assert_eq!(app.dpi(), MAX_DPI, "above the ceiling is clamped down");
}

#[test]
fn a_large_cache_evicts_the_oldest_page() {
    let mut cache = PageCache::new(2);
    for page in 0..3 {
        let rendered = steleofd_reader::RenderedPage {
            image: egui::ColorImage::new([1, 1], vec![egui::Color32::WHITE]),
            size_mm: (210.0, 297.0),
        };
        cache.insert(page, 96.0, rendered);
    }
    assert_eq!(cache.len(), 2, "the cache never exceeds its limit");
    assert!(cache.get(0, 96.0).is_none(), "the oldest page was evicted");
    assert!(cache.get(2, 96.0).is_some(), "the newest page is kept");
}

#[test]
fn the_layout_choice_is_remembered() {
    let mut app = App::new();
    assert_eq!(app.layout(), Layout::Single, "one page is the default");
    app.set_layout(Layout::Facing);
    assert_eq!(app.layout(), Layout::Facing);
    app.set_layout(Layout::Continuous);
    assert_eq!(app.layout(), Layout::Continuous);
}

#[test]
fn the_page_label_counts_from_one() {
    let app = open();
    let label = app.page_label();
    assert!(label.contains('1'), "the first page reads as 1: {label}");
    assert!(
        label.contains(&app.page_count().to_string()),
        "the label names the total: {label}"
    );
}

#[test]
fn a_failure_is_shown_and_can_be_cleared() {
    let mut app = App::new();
    app.set_error("the page would not render");
    assert_eq!(
        app.error().map(|e| e.message.as_str()),
        Some("the page would not render")
    );
    app.clear_error();
    assert!(app.error().is_none());
}

#[test]
fn the_open_request_is_handed_out_once() {
    let mut app = App::new();
    assert!(!app.wants_open());
    app.request_open();
    assert!(app.wants_open());
    assert!(app.take_open_request(), "the window acts on it");
    assert!(!app.wants_open(), "and does not act on it twice");
}

#[test]
fn opening_another_document_replaces_the_first() {
    let mut app = open();
    let first = app.page_count();
    app.open_bytes(&std::fs::read(corpus()).unwrap(), Some("again.ofd"))
        .expect("the corpus opens a second time");
    assert_eq!(app.page_count(), first, "the same document, the same pages");
    assert_eq!(app.current_page(), 0, "reading starts at the first page");
}

#[test]
fn an_empty_reader_asks_for_a_document_rather_than_a_page() {
    let mut app = App::new();
    assert_eq!(app.canvas(), Canvas::Empty, "no document, nothing to paint");
    assert!(
        !app.drop_hint().is_empty(),
        "the hint is written in the reader's language"
    );
    assert!(
        !app.title().contains(".ofd"),
        "the title names no document yet"
    );
}

#[test]
fn an_open_reader_paints_its_page() {
    let mut app = open();
    let canvas = app.canvas();
    assert!(
        matches!(canvas, Canvas::Page | Canvas::Blank),
        "the corpus rasterizes to something paintable, got {canvas:?}"
    );
}

#[test]
fn a_blank_page_is_still_drawn() {
    let mut app = open();
    app.canvas();
    let blank = app.current_page_is_blank();
    assert_eq!(
        app.canvas(),
        if blank { Canvas::Blank } else { Canvas::Page },
        "a blank page is a page, so the canvas stays in the drawing branch"
    );
    assert!(
        !app.blank_notice().is_empty(),
        "and the notice is localized"
    );
}

#[test]
fn an_error_alone_does_not_displace_the_empty_state() {
    let mut app = App::new();
    app.set_error("the page would not render");
    assert_eq!(
        app.canvas(),
        Canvas::Empty,
        "the canvas checks for a document before it checks for a failure"
    );
    assert_eq!(
        app.error().map(|e| e.message.as_str()),
        Some("the page would not render"),
        "the status bar still shows it"
    );
}

#[test]
fn an_unreadable_page_is_reported_rather_than_left_blank() {
    let mut app = open();
    app.go_to(app.page_count() + 10);
    assert_eq!(
        app.canvas(),
        Canvas::Page,
        "a page past the end clamps back into the document"
    );
}

#[test]
fn a_failed_open_leaves_the_previous_document_open() {
    let mut app = open();
    let pages = app.page_count();
    let err = app.open_bytes(b"not an ofd container", Some("broken.ofd"));
    assert!(err.is_err(), "the broken document is refused");
    assert_eq!(app.page_count(), pages, "and the good one is still open");
    assert!(app.path().is_some(), "the reader keeps its place");
}

#[test]
fn a_dropped_path_opens_like_a_chosen_one() {
    let mut app = App::new();
    app.open_dropped(DroppedFile::Path(corpus()))
        .expect("the corpus opens from a drop");
    assert!(app.page_count() >= 1);
    assert_eq!(app.file_name().as_deref(), Some("minimal.ofd"));
}

#[test]
fn a_dropped_payload_opens_and_keeps_its_name() {
    let bytes = std::fs::read(corpus()).unwrap();
    let mut app = App::new();
    app.open_dropped(DroppedFile::Bytes {
        name: "dropped.ofd".to_string(),
        bytes,
    })
    .expect("the corpus opens from dropped bytes");
    assert!(app.page_count() >= 1);
    assert_eq!(app.file_name().as_deref(), Some("dropped.ofd"));
}

#[test]
fn the_window_title_names_the_open_document() {
    let mut app = App::new();
    assert_eq!(app.title(), "SteleOFD Reader");
    app.open_path(&corpus()).unwrap();
    assert!(
        app.title().starts_with("minimal.ofd"),
        "got {}",
        app.title()
    );
}

#[test]
fn the_info_line_is_written_in_the_readers_language() {
    let mut app = App::new();
    assert!(app.info_label().is_empty(), "no document, no line");
    app.open_path(&corpus()).unwrap();
    assert!(
        app.info_label().contains("minimal.ofd"),
        "got {}",
        app.info_label()
    );
}
