//! The window, driven headlessly: a real widget tree, no display. A label that
//! loses its catalogue entry or a button that stops wiring to `App` fails here,
//! not in front of a reader. Labels come from the catalogue, not literals.

use std::path::PathBuf;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use steleofd_i18n::{I18n, Locale, MessageId};
use steleofd_reader::viewer::Layout;
use steleofd_reader::window;
use steleofd_reader::{App, Canvas, Fit};

/// The real minimal document from the conformance corpus.
fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../compliance/corpus/minimal.ofd")
}

/// An empty reader behind a harness, sized like a small window.
fn harness(app: App) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .build_ui_state(
            |ui, app: &mut App| {
                window::draw(ui, app);
            },
            app,
        );
    harness.run();
    harness
}

/// The label the catalogue gives a key, so the test does not hardcode English.
fn label(key: MessageId) -> String {
    I18n::with_locale(Locale::en_us()).text(key)
}

/// Run frames until the pages are rasterized, since rendering is off the draw
/// path and a frame is what collects a finished raster.
fn pump_until_rendered(harness: &mut Harness<'static, App>, pages: &[usize]) {
    // Rendering is off the draw path, so a frame collects a finished raster;
    // the budget is wall clock because a shared runner can stall the worker.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        harness.run();
        if pages
            .iter()
            .all(|page| harness.state().cached(*page).is_some())
        {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("pages {pages:?} never rendered");
}

#[test]
fn the_menu_bar_shows_a_localized_open_button() {
    let harness = harness(App::new());
    let open = label(MessageId::ReaderOpen);
    // Panics with a list of what did exist if the button is not drawn.
    harness.get_by_label(&open);
}

#[test]
fn the_toolbar_labels_every_control() {
    let harness = harness(App::new());
    for key in [MessageId::ReaderFitWidth, MessageId::ReaderFitPage] {
        harness.get_by_label_contains(&label(key));
    }
    // The zoom control shows the level, not the template, so it is found by
    // the substituted text a reader actually sees. A slider exposes two nodes
    // with that label, so the query asks for the slider role.
    let zoom = harness.get_by(|node| {
        node.role() == egui::accesskit::Role::Slider
            && node.label().unwrap_or_default().starts_with("Zoom")
    });
    assert_eq!(zoom.accesskit_node().numeric_value(), Some(96.0));
}

#[test]
fn the_layout_choices_are_offered_and_one_is_selected() {
    let mut harness = harness(App::new());
    let single = label(MessageId::ReaderLayoutSingle);
    let facing = label(MessageId::ReaderLayoutDouble);
    let continuous = label(MessageId::ReaderLayoutContinuous);
    for name in [&single, &facing, &continuous] {
        harness.get_by_label(name);
    }
    // Clicking one moves the selection, which the state machine then remembers.
    harness.get_by_label(&facing).click();
    harness.run();
    assert_eq!(
        harness.state().layout(),
        steleofd_reader::viewer::Layout::Facing
    );
}

#[test]
fn an_empty_window_offers_the_drop_hint_instead_of_a_page() {
    let mut harness = harness(App::new());
    let hint = label(MessageId::ReaderDropHint);
    harness.get_by_label_contains(&hint);
    assert_eq!(harness.state_mut().canvas(), Canvas::Empty);
}

#[test]
fn opening_the_corpus_draws_its_page_counter() {
    let mut app = App::new();
    app.open_path(&corpus()).expect("the corpus opens");
    let harness = harness(app);

    // The status bar names the document and counts the pages.
    harness.get_by_label_contains("minimal.ofd");
    harness.get_by_label_contains("1 of");
    assert!(harness.state().page_count() >= 1);
}

#[test]
fn the_next_button_turns_the_page_and_stops_at_the_end() {
    let mut app = App::new();
    app.open_path(&corpus()).expect("the corpus opens");
    let mut harness = harness(app);

    let next = "▶";
    let before = harness.state().current_page();
    if harness.state().has_next() {
        harness.get_by_label(next).click();
        harness.run();
        assert_eq!(
            harness.state().current_page(),
            before + 1,
            "the toolbar button advanced one page"
        );
    }
    // Walk to the end; the button must be disabled there rather than absent.
    while harness.state().has_next() {
        harness.get_by_label(next).click();
        harness.run();
    }
    assert!(
        !harness.state().has_next(),
        "the reader is at the last page"
    );
    harness.get_by_label(next);
}

#[test]
fn the_close_button_is_disabled_until_a_document_is_open() {
    let harness = harness(App::new());
    let close = harness.state().close_label();
    let button = harness.get_by_label(&close);
    assert!(
        button.accesskit_node().is_disabled(),
        "an empty reader has nothing to close, so the control is disabled"
    );
}

#[test]
fn closing_through_the_menu_returns_the_window_to_the_hint() {
    let mut app = App::new();
    app.open_path(&corpus()).expect("the corpus opens");
    let mut harness = harness(app);

    let close = harness.state().close_label();
    harness.get_by_label(&close).click();
    harness.run();

    assert_eq!(
        harness.state_mut().canvas(),
        Canvas::Empty,
        "the page is gone"
    );
    let hint = label(MessageId::ReaderDropHint);
    harness.get_by_label_contains(&hint);
}

#[test]
fn a_failure_is_drawn_in_the_status_bar() {
    let mut app = App::new();
    app.open_path(&corpus()).expect("the corpus opens");
    app.set_error("the page would not render");
    let harness = harness(app);
    harness.get_by_label_contains("the page would not render");
}

#[test]
fn the_fit_buttons_choose_a_fit_and_it_sticks() {
    let mut app = App::new();
    app.open_path(&corpus()).expect("the corpus opens");
    let mut harness = harness(app);

    // A fit is a mode, not a one-off resolution: choosing one has to be
    // visible to the toolbar so it can show which is in force.
    assert_eq!(harness.state().fit(), None, "nothing fitted on open");

    let fit_page = label(MessageId::ReaderFitPage);
    harness.get_by_label(&fit_page).click();
    harness.run();
    assert_eq!(
        harness.state().fit(),
        Some(Fit::Page),
        "fit page is the mode in force"
    );

    let fit_width = label(MessageId::ReaderFitWidth);
    harness.get_by_label(&fit_width).click();
    harness.run();
    assert_eq!(harness.state().fit(), Some(Fit::Width));
}

#[test]
fn a_fit_recomputes_the_resolution_from_the_page_and_the_window() {
    let mut app = App::new();
    app.open_path(&corpus()).expect("the corpus opens");
    let mut harness = harness(app);

    harness.state_mut().set_fit(Fit::Width);
    harness.run();
    let wide = harness.state().dpi();

    // The same page in a narrower window must be shown smaller; a fixed
    // constant would not move at all.
    let mut narrow = Harness::builder()
        .with_size(egui::vec2(400.0, 700.0))
        .build_ui_state(
            |ui, app: &mut App| {
                window::draw(ui, app);
            },
            App::new(),
        );
    narrow.state_mut().set_fit(Fit::Width);
    narrow.run();
    let fitted = narrow.state().dpi();
    assert!(
        fitted < wide,
        "a narrower window fits at a lower resolution: {fitted} vs {wide}"
    );
}

#[test]
fn facing_layout_draws_both_pages_of_the_spread() {
    // Two pages on screen means two images; single layout means one. Counting
    // image nodes is what proves the layout reaches the canvas, since the
    // canvas is the only zone allowed to paint a page.
    let bytes = steleofd_testkit::multipage_archive(4);
    let mut app = App::new();
    app.open_bytes(&bytes, Some("many.ofd"))
        .expect("the fixture opens");
    let mut harness = harness(app);

    let images = |harness: &Harness<'static, App>| {
        harness
            .query_all_by(|node| node.role() == egui::accesskit::Role::Image)
            .count()
    };

    harness.state_mut().set_layout(Layout::Single);
    harness.state_mut().page_image(0).expect("page 0 requested");
    pump_until_rendered(&mut harness, &[0]);
    assert_eq!(images(&harness), 1, "single layout shows one page");

    harness.state_mut().set_layout(Layout::Facing);
    harness.state_mut().set_dpi(96.0);
    for page in [0, 1] {
        harness
            .state_mut()
            .page_image(page)
            .expect("page requested");
    }
    pump_until_rendered(&mut harness, &[0, 1]);
    assert_eq!(images(&harness), 2, "facing layout shows the whole spread");
}

#[test]
fn the_continuous_layout_stacks_more_than_one_page() {
    let bytes = steleofd_testkit::multipage_archive(9);
    let mut app = App::new();
    app.open_bytes(&bytes, Some("many.ofd"))
        .expect("the fixture opens");
    let mut harness = harness(app);
    harness.state_mut().set_layout(Layout::Continuous);
    harness.state_mut().go_to(4);
    let wanted = harness.state().visible();
    for page in &wanted {
        harness
            .state_mut()
            .page_image(*page)
            .expect("page requested");
    }
    pump_until_rendered(&mut harness, &wanted);
    let images = harness
        .query_all_by(|node| node.role() == egui::accesskit::Role::Image)
        .count();
    assert!(
        images > 1,
        "a continuous view shows its window, got {images}"
    );
}

#[test]
fn the_search_panel_is_closed_until_it_is_asked_for() {
    let mut app = App::new();
    app.open_path(&corpus()).expect("the corpus opens");
    let mut harness = harness(app);

    // The toggle exists, because the bar does not reflow; the panel does not,
    // because it would take space the page needs.
    let title = label(MessageId::ReaderSearchTitle);
    harness.get_by_label(&title);
    assert!(!harness.state().search_open(), "closed on open");

    harness.get_by_label(&title).click();
    harness.run();
    assert!(harness.state().search_open(), "the toggle opened it");

    harness.get_by_label(&title).click();
    harness.run();
    assert!(!harness.state().search_open(), "and closes it again");
}

#[test]
fn typing_a_query_shows_the_hits_and_the_count() {
    let mut app = App::new();
    app.open_path(&corpus()).expect("the corpus opens");
    app.set_search_open(true);
    let mut harness = harness(app);

    // The count comes from the catalogue with the number substituted, so the
    // test asserts on the localized sentence rather than on a literal.
    harness.state_mut().set_query("SteleOFD");
    harness.run();
    assert!(
        !harness.state().hits().is_empty(),
        "the corpus has the word"
    );

    let count = harness.state().search_count_label();
    harness.get_by_label_contains(&count);
}

#[test]
fn a_query_with_no_match_offers_the_no_match_notice() {
    let mut app = App::new();
    app.open_path(&corpus()).expect("the corpus opens");
    app.set_search_open(true);
    app.set_query("zzzz-not-in-this-document");
    let harness = harness(app);

    let notice = label(MessageId::ReaderNoMatch);
    harness.get_by_label_contains(&notice);
}

#[test]
fn the_search_toggle_is_disabled_until_a_document_is_open() {
    // There is nothing to search with no document open, so the toggle is
    // disabled rather than hidden. This mirrors the close button's rule.
    let harness = harness(App::new());
    let title = label(MessageId::ReaderSearchTitle);
    let button = harness.get_by_label(&title);
    assert!(
        button.accesskit_node().is_disabled(),
        "with nothing open the search toggle does nothing"
    );
}

#[test]
fn the_signature_toggle_is_disabled_until_a_document_is_open() {
    let harness = harness(App::new());
    let heading = label(MessageId::SignPanelTitle);
    let button = harness.get_by_label(&heading);
    assert!(
        button.accesskit_node().is_disabled(),
        "there is nothing to verify with no document open"
    );
}

#[test]
fn opening_the_signature_panel_shows_the_count_and_the_verdict() {
    let mut app = App::new();
    let (bytes, _key) = steleofd_testkit::signed_fixture();
    app.open_bytes(&bytes, Some("signed.ofd"))
        .expect("the signed fixture opens");
    let mut harness = harness(app);

    let heading = label(MessageId::SignPanelTitle);
    assert!(
        harness.query_by_label_contains(&heading).is_some(),
        "the toggle is in the toolbar"
    );
    harness.get_by_label(&heading).click();
    harness.run();
    assert!(harness.state().signatures_open(), "the toggle opened it");

    // The count is the catalogue's plural sentence with the number in it.
    let count = harness.state().signature_count_label();
    harness.get_by_label(&count);
    // And the verdict is the word for a signature that holds.
    let verdict = label(MessageId::SignValid);
    harness.get_by_label_contains(&verdict);
}

#[test]
fn the_export_buttons_are_disabled_until_a_document_is_open() {
    let harness = harness(App::new());
    for key in [MessageId::ReaderExportPdf, MessageId::ReaderExportPng] {
        let text = label(key);
        let button = harness.get_by_label(&text);
        assert!(
            button.accesskit_node().is_disabled(),
            "there is nothing to export with no document open"
        );
    }
}

#[test]
fn asking_to_export_asks_the_host_where_to_write_it() {
    // The window cannot know where a person wants a file, so it reports the
    // format and lets the host ask the platform.
    let mut app = App::new();
    let bytes = steleofd_testkit::multipage_archive(3);
    app.open_bytes(&bytes, Some("many.ofd"))
        .expect("the fixture opens");
    let mut harness = harness(app);

    harness
        .get_by_label(&label(MessageId::ReaderExportPdf))
        .click();
    harness.run();
    assert_eq!(
        harness.state().wants_export(),
        Some(steleofd_reader::ExportFormat::Pdf),
        "the state holds the request until the host answers"
    );
}

#[test]
fn a_finished_export_is_announced_in_the_status_bar() {
    let dir = std::env::temp_dir().join("steleofd-window-export");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory is creatable");
    let out = dir.join("out.pdf");
    let mut app = App::new();
    let bytes = steleofd_testkit::multipage_archive(2);
    app.open_bytes(&bytes, Some("many.ofd"))
        .expect("the fixture opens");
    app.export_to(steleofd_reader::ExportFormat::Pdf, vec![0], out.clone());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        app.poll();
        if !app.is_exporting() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let harness = harness(app);
    let label = harness.state().export_label().expect("the export reported");
    harness.get_by_label_contains(&label);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn asking_to_print_reaches_the_host_without_a_destination() {
    // Printing is bounded by the paper the reader has, not by a file, so the
    // window reports the request and does not wait for a dialog.
    let mut app = App::new();
    let bytes = steleofd_testkit::multipage_archive(3);
    app.open_bytes(&bytes, Some("many.ofd"))
        .expect("the fixture opens");
    let mut harness = harness(app);

    harness.get_by_label(&label(MessageId::ReaderPrint)).click();
    harness.run();
    assert!(
        harness.state().wants_print(),
        "the print button did not reach the state the host reads"
    );
}

#[test]
fn the_print_button_is_disabled_until_a_document_is_open() {
    let harness = harness(App::new());
    let text = label(MessageId::ReaderPrint);
    let button = harness.get_by_label(&text);
    assert!(
        button.accesskit_node().is_disabled(),
        "there is nothing to print with no document open"
    );
}

#[test]
fn a_finished_print_job_is_announced_in_the_status_bar() {
    let mut app = App::new();
    let bytes = steleofd_testkit::multipage_archive(2);
    app.open_bytes(&bytes, Some("many.ofd"))
        .expect("the fixture opens");
    app.print_to(app.print_pages(), app.print_settings());
    // A shared runner can stall the print worker, so wait on the clock rather
    // than on a fixed iteration count; a real hang still fails the assert.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        app.poll();
        if !app.is_printing() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let harness = harness(app);
    let label = harness.state().print_label().expect("the job reported");
    harness.get_by_label_contains(&label);
}

#[test]
fn the_annotation_toggle_is_disabled_until_a_document_is_open() {
    let harness = harness(App::new());
    let text = label(MessageId::AnnotPanelTitle);
    let button = harness.get_by_label(&text);
    assert!(
        button.accesskit_node().is_disabled(),
        "there is nothing to list with no document open"
    );
}

#[test]
fn opening_the_annotation_panel_shows_the_count_and_the_marks() {
    let mut app = App::new();
    let bytes = steleofd_testkit::annotated_archive(3, 2);
    app.open_bytes(&bytes, Some("marked.ofd"))
        .expect("the fixture opens");
    let mut harness = harness(app);

    harness
        .get_by_label(&label(MessageId::AnnotPanelTitle))
        .click();
    harness.run();
    assert!(harness.state().annotations_open(), "the toggle opened it");

    // The count is the catalogue's plural sentence with the number in it.
    let count = harness.state().annotation_count_label();
    harness.get_by_label(&count);
    // And the row is the mark itself, which a reader has to be able to see.
    let row = harness.state().annotation_label(0);
    harness.get_by_label_contains(&row);
}

#[test]
fn a_document_with_no_marks_says_so_in_the_panel() {
    // A document without annotations is not a failure, so the panel states a
    // fact rather than leaving an empty column.
    let mut app = App::new();
    let bytes = steleofd_testkit::multipage_archive(2);
    app.open_bytes(&bytes, Some("plain.ofd"))
        .expect("the fixture opens");
    let mut harness = harness(app);

    harness
        .get_by_label(&label(MessageId::AnnotPanelTitle))
        .click();
    harness.run();
    harness.get_by_label(&label(MessageId::AnnotNone));
}

#[test]
fn the_menu_offers_both_languages_in_their_own_names() {
    // A reader who cannot read the menu's current language still has to be
    // able to find theirs, so the entries are endonyms rather than translations.
    let harness = harness(App::new());
    harness.get_by_label("English");
    harness.get_by_label(&label(MessageId::LocaleNameZh));
}

#[test]
fn choosing_a_language_redraws_the_window_in_it() {
    let mut app = App::new();
    let bytes = steleofd_testkit::multipage_archive(3);
    app.open_bytes(&bytes, Some("many.ofd"))
        .expect("the fixture opens");
    let mut harness = harness(app);

    harness
        .get_by_label(&label(MessageId::LocaleNameZh))
        .click();
    harness.run();
    assert_eq!(harness.state().locale().tag(), "zh-CN");

    // The page counter is a catalogue string, so it follows the language rather
    // than staying English behind a translated menu.
    let label = harness.state().page_label();
    // The catalogue entry for the Chinese page counter carries the character
    // for "page", written as an escape because the source carries no literal.
    assert!(
        label.contains('\u{9875}'),
        "the page label did not follow: {label}"
    );
    harness.get_by_label_contains(&label);
}

#[test]
fn the_comparison_panel_lists_a_changed_page() {
    // Opening the panel and comparing two revisions has to show the verdict
    // and one row per changed page, all from the catalogue.
    let mut app = App::new();
    let one = steleofd_testkit::multipage_archive(1);
    app.open_bytes(&one, Some("rev-a.ofd")).expect("opens");
    app.compare_bytes(&steleofd_testkit::multipage_archive(2))
        .expect("compare");
    let harness = harness(app);

    // The toggle and the panel heading share the label, so the heading is not
    // asserted by label here; the toggle test covers it and the summary below
    // proves the panel body is drawn.
    let summary = harness.state().comparison_summary();
    let changed = harness
        .state()
        .comparison()
        .expect("a verdict")
        .changed_pages();
    assert!(changed >= 1, "the second revision added a page");
    assert!(
        summary.contains(&changed.to_string()),
        "the summary states the count: {summary}"
    );
    harness.get_by_label_contains(&summary);
}

#[test]
fn the_comparison_toggle_opens_the_panel() {
    let mut app = App::new();
    let one = steleofd_testkit::multipage_archive(1);
    app.open_bytes(&one, Some("rev-a.ofd")).expect("opens");
    let mut harness = harness(app);
    assert!(!harness.state().comparison_open());

    let heading = label(MessageId::ComparePanelTitle);
    harness.get_by_label(&heading).click();
    harness.run();
    assert!(
        harness.state().comparison_open(),
        "the toggle opens the panel"
    );
}

#[test]
fn a_comparison_that_matches_says_so() {
    let mut app = App::new();
    let one = steleofd_testkit::multipage_archive(1);
    app.open_bytes(&one, Some("rev-a.ofd")).expect("opens");
    app.compare_bytes(&one).expect("compare");
    let harness = harness(app);

    let identical = label(MessageId::CompareIdentical);
    assert_eq!(harness.state().comparison_summary(), identical);
    harness.get_by_label_contains(&identical);
}
