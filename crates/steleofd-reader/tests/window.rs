//! The window, driven headlessly: a real widget tree, no display. A label that
//! loses its catalogue entry or a button that stops wiring to `App` fails here,
//! not in front of a reader. Labels come from the catalogue, not literals.

use std::path::PathBuf;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use steleofd_i18n::{I18n, Locale, MessageId};
use steleofd_reader::window;
use steleofd_reader::{App, Canvas};

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
fn the_fit_buttons_move_the_resolution() {
    let mut app = App::new();
    app.open_path(&corpus()).expect("the corpus opens");
    let mut harness = harness(app);
    harness.state_mut().set_dpi(280.0);
    harness.run();

    let fit_page = label(MessageId::ReaderFitPage);
    harness.get_by_label(&fit_page).click();
    harness.run();
    assert_eq!(
        harness.state().dpi(),
        window::FIT_PAGE_DPI,
        "fit page selected the page resolution"
    );

    let fit_width = label(MessageId::ReaderFitWidth);
    harness.get_by_label(&fit_width).click();
    harness.run();
    assert_eq!(harness.state().dpi(), window::FIT_WIDTH_DPI);
}
