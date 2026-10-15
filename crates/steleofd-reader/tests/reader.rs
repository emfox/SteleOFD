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

/// A reader over a document with `pages` pages, so arrangements have room.
fn open_pages(pages: usize) -> App {
    let mut app = App::new();
    let bytes = steleofd_testkit::multipage_archive(pages);
    app.open_bytes(&bytes, Some("many.ofd"))
        .expect("the fixture opens");
    app
}

/// Ask for a page and wait for the worker to deliver it.
/// Rendering is off the draw path, so a test that wants a raster has to pump
/// the queue the way a frame does; the bound fails loudly if it never answers.
fn wait_for_page(app: &mut App, page: usize) {
    app.page_image(page).expect("the request is accepted");
    for _ in 0..2000 {
        app.poll();
        if app.cached(page).is_some() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("page {page} never arrived from the render worker");
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

    wait_for_page(&mut app, page);
    let first = app.cached(page).expect("the page is now cached").image.size;
    assert!(
        first[0] > 0 && first[1] > 0,
        "the image has real dimensions"
    );

    // A second request must not rasterize again: the cache answers it.
    app.page_image(page).expect("the cached page is returned");
    assert_eq!(app.cached(page).map(|p| p.image.size), Some(first));
}

/// The reader asks for what is on screen once per frame, so a page already on
/// its way must not be asked for again. Restamping it every frame would leave
/// the worker forever rendering a raster the next frame had superseded.
#[test]
fn asking_for_the_same_page_every_frame_still_delivers_it() {
    let mut app = open();
    let page = app.current_page();
    for _ in 0..2000 {
        // What a frame does: ask for the visible page, then collect results.
        let _ = app.canvas();
        app.poll();
        if app.cached(page).is_some() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("page {page} was asked for on every frame and never arrived");
}

#[test]
fn the_cache_keys_on_the_resolution_too() {
    let mut app = open();
    let page = app.current_page();
    wait_for_page(&mut app, page);
    let wide = app.cached(page).map(|p| p.image.size).unwrap();

    app.set_dpi(200.0);
    wait_for_page(&mut app, page);
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
fn an_open_reader_asks_the_worker_then_paints_its_page() {
    let mut app = open();
    let page = app.current_page();
    // The draw path never rasterizes: asked for a page it has not got, it
    // reports it as on its way, or hands back the page it already has if the
    // worker was quick enough to beat the question.
    let first = app.canvas();
    assert!(
        matches!(first, Canvas::Pending | Canvas::Page | Canvas::Blank),
        "the canvas either waits or paints, got {first:?}"
    );

    wait_for_page(&mut app, page);
    let canvas = app.canvas();
    assert!(
        matches!(canvas, Canvas::Page | Canvas::Blank),
        "the corpus rasterizes to something paintable, got {canvas:?}"
    );
    assert!(!app.is_rendering(), "and the worker owes nothing more");
}

#[test]
fn a_blank_page_is_still_drawn() {
    let mut app = open();
    let page = app.current_page();
    wait_for_page(&mut app, page);
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
fn a_page_past_the_end_clamps_back_into_the_document() {
    let mut app = open();
    app.go_to(app.page_count() + 10);
    let page = app.current_page();
    wait_for_page(&mut app, page);
    assert_eq!(
        app.canvas(),
        Canvas::Page,
        "the clamped page still rasterizes rather than failing"
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

#[test]
fn a_page_never_rasterizes_past_the_texture_limit() {
    // A4 at the reader's highest zoom would be 3,480 px wide, which no driver
    // will hold, so the resolution has to fall back to what actually fits.
    let a4 = (210.0, 297.0);
    let capped = steleofd_reader::viewer::dpi_that_fits(a4);
    assert!(
        capped < MAX_DPI,
        "A4 must be capped below the requested ceiling, got {capped}"
    );
    let side = f64::from(capped) / 25.4 * 297.0;
    assert!(
        side <= f64::from(steleofd_reader::viewer::MAX_TEXTURE_SIDE),
        "the long side is {side:.0} px, past the limit"
    );
}

#[test]
fn a_rendered_page_is_small_enough_to_upload() {
    let mut app = open();
    app.set_dpi(MAX_DPI);
    wait_for_page(&mut app, 0);
    let rendered = app.cached(0).expect("and lands in the cache");
    let (w, h) = (rendered.image.width(), rendered.image.height());
    let limit = steleofd_reader::viewer::MAX_TEXTURE_SIDE as usize;
    assert!(
        w <= limit && h <= limit,
        "rasterized {w}x{h}, limit {limit}"
    );
}

#[test]
fn a_small_page_may_use_the_full_zoom_range() {
    // Nothing is capped for a page small enough to fit at the top zoom.
    let postage_stamp = (20.0, 20.0);
    assert_eq!(
        steleofd_reader::viewer::dpi_that_fits(postage_stamp),
        MAX_DPI,
        "a small page is not penalised for the limit"
    );
}

#[test]
fn single_page_layout_shows_exactly_the_current_page() {
    let mut app = open_pages(5);
    app.set_layout(Layout::Single);
    for page in 0..5 {
        app.go_to(page);
        assert_eq!(app.visible(), vec![page], "single shows only page {page}");
    }
}

#[test]
fn facing_layout_pairs_pages_and_keeps_the_pairing_stable() {
    let mut app = open_pages(6);
    app.set_layout(Layout::Facing);

    // A spread is the fixed pair (even, even+1), so page 0 always sits with 1
    // and the pairing does not change as the reader moves through it.
    app.go_to(0);
    assert_eq!(app.visible(), vec![0, 1]);
    app.go_to(1);
    assert_eq!(app.visible(), vec![0, 1], "1 stays with 0, not with 2");
    app.go_to(2);
    assert_eq!(app.visible(), vec![2, 3]);
    app.go_to(3);
    assert_eq!(app.visible(), vec![2, 3], "3 stays with 2");
    app.go_to(5);
    assert_eq!(app.visible(), vec![4, 5]);
}

#[test]
fn a_trailing_odd_page_is_a_spread_of_one() {
    let mut app = open_pages(5);
    app.set_layout(Layout::Facing);
    app.go_to(4);
    assert_eq!(app.visible(), vec![4], "the last page has no partner");
}

#[test]
fn continuous_layout_shows_a_bounded_window_around_the_current_page() {
    let mut app = open_pages(20);
    app.set_layout(Layout::Continuous);

    app.go_to(10);
    let window = app.visible();
    assert!(
        window.contains(&10),
        "the current page is always in the window"
    );
    assert!(
        window.len() < 20,
        "continuous must not rasterize the whole document, got {} pages",
        window.len()
    );
    assert_eq!(window.first(), Some(&7), "three pages of lead-in");
    assert_eq!(window.last(), Some(&13), "three pages of look-ahead");

    app.go_to(0);
    assert_eq!(
        app.visible().first(),
        Some(&0),
        "the window clamps at the start"
    );
    app.go_to(19);
    assert_eq!(app.visible().last(), Some(&19), "and at the end");
}

#[test]
fn a_layout_with_no_pages_shows_nothing() {
    let mut app = App::new();
    for layout in [Layout::Single, Layout::Facing, Layout::Continuous] {
        app.set_layout(layout);
        assert!(app.visible().is_empty(), "{layout:?} on an empty reader");
    }
}

#[test]
fn facing_layout_steps_a_spread_at_a_time() {
    let mut app = open_pages(6);
    app.set_layout(Layout::Facing);

    app.go_to(0);
    app.next_page();
    assert_eq!(
        app.current_page(),
        2,
        "a spread advances to the next spread"
    );
    app.previous_page();
    assert_eq!(app.current_page(), 0, "and back again");

    app.go_to(4);
    assert!(!app.has_next(), "the last spread has nothing after it");
    assert!(app.has_previous());
}

#[test]
fn single_page_layout_still_steps_one_page() {
    let mut app = open_pages(3);
    app.set_layout(Layout::Single);
    app.go_to(0);
    app.next_page();
    assert_eq!(app.current_page(), 1);
    app.previous_page();
    assert_eq!(app.current_page(), 0);
    assert!(!app.has_previous(), "page 0 has nothing before it");
}

#[test]
fn changing_layout_keeps_the_page_the_reader_was_on() {
    let mut app = open_pages(6);
    app.go_to(3);
    app.set_layout(Layout::Facing);
    // Page 3 belongs to the spread [2, 3], so it is still on screen.
    assert!(app.visible().contains(&3), "the reader keeps its place");
    app.set_layout(Layout::Single);
    assert_eq!(app.current_page(), 3);
}
