//! N4-d: the font layer driven as a consumer.
//! A real face is loaded from the shared fixture, so lookup, shaping, metrics
//! and outlining run for real, plus the missing-font error a converter checks.

use std::sync::Arc;

use steleofd_font::{FontId, FontManager, ShapeOptions, shape_text};

/// The shared real face, so outlining and shaping are not degraded paths.
fn font_bytes() -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../steleofd-render/tests/data/Orbitron-Bold.ttf"
    );
    std::fs::read(path).expect("font fixture")
}

/// A manager with the fixture face registered under id 0.
fn manager_with_fixture() -> (FontManager, FontId) {
    let mut manager = FontManager::new();
    let id = manager.registry_mut().register_embedded(
        "Res/Orbitron-Bold.ttf",
        "Orbitron",
        Arc::new(font_bytes()),
    );
    (manager, id)
}

#[test]
fn an_embedded_face_loads_and_reports_its_family() {
    let (manager, id) = manager_with_fixture();
    let face = manager.face(id).expect("load");
    assert_eq!(face.family(), "Orbitron");
    assert_eq!(face.id(), id);
    assert!(!face.is_fallback());
    assert!(face.units_per_em() > 0.0);
}

#[test]
fn a_face_reports_metrics_a_caller_can_lay_out_with() {
    let (manager, id) = manager_with_fixture();
    let metrics = manager.face(id).expect("load").metrics();
    // Ascent sits above the baseline and descent below; both are non-zero for
    // any real face, and the line height combines them with the gap.
    assert!(metrics.ascender > 0.0, "{metrics:?}");
    assert!(metrics.units_per_em > 0);
    assert_eq!(
        metrics.line_height(),
        metrics.ascender - metrics.descender + metrics.line_gap
    );
}

#[test]
fn a_known_codepoint_maps_to_a_glyph_and_back() {
    let (manager, id) = manager_with_fixture();
    let face = manager.face(id).expect("load");
    let glyph = face.glyph_id('A' as u32).expect("A is in the fixture");
    assert_ne!(glyph, 0, "glyph 0 is .notdef");
    // The reverse map is what search and copy depend on, so the round trip has
    // to land back on the character it started from.
    assert_eq!(face.reverse_cmap(glyph), Some('A' as u32));
}

#[test]
fn a_character_the_face_lacks_is_absent_rather_than_wrong() {
    let (manager, id) = manager_with_fixture();
    let face = manager.face(id).expect("load");
    // A CJK ideograph is not in a Latin display face, and the honest answer is
    // `None` rather than a bogus glyph.
    assert!(!face.has_codepoint('\u{4e2d}' as u32));
    assert_eq!(face.glyph_id('\u{4e2d}' as u32), None);
}

#[test]
fn a_real_glyph_has_an_outline_and_a_space_does_not() {
    let (manager, id) = manager_with_fixture();
    let face = manager.face(id).expect("load");

    let a = face.glyph_id('A' as u32).expect("A");
    assert!(
        face.outline_for_codepoint('A' as u32).is_some(),
        "A has an outline"
    );
    assert!(matches!(
        face.glyph_outline(a),
        steleofd_font::GlyphOutline::Path(_)
    ));

    // A space has no ink, so its outline is empty; a caller that wrote a path
    // for it would emit a stray mark.
    if let Some(space) = face.glyph_id(' ' as u32) {
        assert!(matches!(
            face.glyph_outline(space),
            steleofd_font::GlyphOutline::Empty
        ));
    }
}

#[test]
fn a_glyph_advance_is_positive_for_a_drawn_character() {
    let (manager, id) = manager_with_fixture();
    let face = manager.face(id).expect("load");
    let a = face.glyph_id('A' as u32).expect("A");
    assert!(face.advance_width(a) > 0.0, "a letter must advance the pen");
}

#[test]
fn an_unknown_font_id_is_an_error_not_a_panic() {
    let (manager, _) = manager_with_fixture();
    let err = manager.face(FontId(999)).expect_err("no such font");
    assert!(err.to_string().contains("999"), "{err}");
}

#[test]
fn shaping_a_string_produces_one_glyph_per_character() {
    let (manager, id) = manager_with_fixture();
    let run = shape_text(&manager, id, "ABC", &ShapeOptions::default()).expect("shape");
    assert_eq!(run.glyphs.len(), 3);
    assert_eq!(run.text, "ABC");
    assert!(run.total_advance > 0.0);
    assert!(run.skipped_chars.is_empty());
    // The clusters are character indices in source order, which is what a
    // selection needs to map a glyph back to a caret position.
    assert_eq!(run.clusters(), vec![0, 1, 2]);
}

#[test]
fn shaping_an_empty_string_is_an_empty_run() {
    let (manager, id) = manager_with_fixture();
    let run = shape_text(&manager, id, "", &ShapeOptions::default()).expect("shape");
    assert!(run.is_empty());
    assert_eq!(run.total_advance, 0.0);
}

#[test]
fn a_character_with_no_face_is_recorded_as_skipped() {
    let (manager, id) = manager_with_fixture();
    // Fallback is off and the fixture lacks the ideograph, so it cannot be
    // drawn; the run reports it rather than silently emitting a blank.
    let options = ShapeOptions {
        allow_fallback: false,
        ..Default::default()
    };
    let run = shape_text(&manager, id, "A\u{4e2d}", &options).expect("shape");
    assert_eq!(run.glyphs.len(), 1, "only the 'A' can be shaped");
    assert_eq!(run.skipped_chars, vec!['\u{4e2d}']);
    assert!(run.has_missing_glyphs());
}

#[test]
fn the_size_in_millimetres_scales_the_advance() {
    let (manager, id) = manager_with_fixture();
    let small = shape_text(
        &manager,
        id,
        "A",
        &ShapeOptions {
            size_mm: 3.5,
            ..Default::default()
        },
    )
    .expect("small");
    let large = shape_text(
        &manager,
        id,
        "A",
        &ShapeOptions {
            size_mm: 7.0,
            ..Default::default()
        },
    )
    .expect("large");
    // Doubling the size doubles the advance, which is the only thing the size
    // is allowed to change.
    let ratio = large.total_advance / small.total_advance;
    assert!((ratio - 2.0).abs() < 0.01, "ratio was {ratio}");
}

#[test]
fn the_fallback_chain_round_trips_and_is_empty_by_default() {
    let (manager, id) = manager_with_fixture();
    assert!(manager.fallback_chain().is_empty());
    manager.set_fallback_chain(vec![id]);
    assert_eq!(manager.fallback_chain(), vec![id]);
}

#[test]
fn a_fallback_is_found_for_a_codepoint_the_chain_covers() {
    let (manager, id) = manager_with_fixture();
    manager.set_fallback_chain(vec![id]);
    let (found, _face) = manager.fallback_for('A' as u32).expect("A is covered");
    assert_eq!(found, id);
    // A codepoint no face in the chain covers yields nothing rather than a
    // wrong face.
    assert!(manager.fallback_for('\u{4e2d}' as u32).is_none());
}

#[test]
fn registering_the_same_bytes_twice_reuses_the_id() {
    let mut manager = FontManager::new();
    let first = manager.registry_mut().register_embedded(
        "Res/Orbitron-Bold.ttf",
        "Orbitron",
        Arc::new(font_bytes()),
    );
    let again = manager.registry_mut().register_embedded(
        "Res/Copy.ttf",
        "Orbitron Copy",
        Arc::new(font_bytes()),
    );
    assert_eq!(
        again, first,
        "identical bytes must not register a second face"
    );
    assert_eq!(manager.registry().len(), 1);
}

#[test]
fn registering_a_family_twice_returns_the_first_id() {
    let mut manager = FontManager::new();
    let first = manager.registry_mut().register_system("Serif");
    let second = manager.registry_mut().register_system("Serif");
    assert_eq!(first, second);
    assert_eq!(manager.registry().len(), 1);
}

#[test]
fn a_system_family_with_no_resolvable_face_is_reported_missing() {
    let mut manager = FontManager::new();
    let id = manager
        .registry_mut()
        .register_system("No Such Family 12345");
    // A system reference with nothing behind it must surface as the documented
    // missing-font error, which is what a converter checks before rendering.
    let err = manager.face(id).expect_err("unresolvable");
    assert!(err.to_string().contains("No Such Family 12345"), "{err}");
}

#[test]
fn a_system_reference_with_no_data_is_listed_as_missing() {
    let mut manager = FontManager::new();
    manager.registry_mut().register_system("Serif");
    assert_eq!(manager.registry().missing_families(), vec!["Serif"]);
}

#[test]
fn clearing_the_cache_drops_the_count_but_keeps_handles_alive() {
    let (manager, id) = manager_with_fixture();
    let held = manager.face(id).expect("load");
    assert!(manager.loaded_count() >= 1);
    manager.clear_cache();
    assert_eq!(manager.loaded_count(), 0);
    // A handle a caller already holds stays valid, so clearing mid-render is
    // safe for work in flight.
    assert_eq!(held.family(), "Orbitron");
}
