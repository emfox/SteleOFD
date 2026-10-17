//! A CJK substitute on a host that does not ship the family the engine pins:
//! Windows and macOS keep their ideographs elsewhere, and font-kit maps the
//! generic names to Latin faces. Each claim is arranged out of the host's way.

use steleofd_font::{FontConfig, FontId, FontManager};

/// A family name no host installs, used where the point is a missing face.
const ABSENT: &str = "No Such Family 98765";

/// The one code point no installed face carries: an unassigned Plane 16 slot.
const UNASSIGNED: u32 = 0x10_FF_FD;

/// The CJK fixture, for a manager that must find no Chinese at all.
fn cjk_fixture() -> std::sync::Arc<Vec<u8>> {
    std::sync::Arc::new(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/steleofd-cjk-subset.ttf"
        ))
        .expect("cjk fixture"),
    )
}

#[test]
fn a_loaded_substitute_is_searched_again_for_characters_it_lacks() {
    // A cached face the run cannot use must send the search out again rather
    // than be reused. The pinned face covers Latin only, and the one configured
    // family is a name the host lacks, so a miss is the right answer.
    let mut manager = FontManager::new();
    manager.set_config(FontConfig::new([ABSENT], std::iter::empty::<&str>()));
    let wide = manager.registry_mut().register_fallback_with_data(
        "cjk-pinned",
        Some(std::sync::Arc::new(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/data/steleofd-latin-subset.ttf"
            ))
            .expect("latin fixture"),
        )),
    );
    assert!(manager.face_for(wide, &[b'A' as u32]).is_ok());
    assert!(
        manager.face_for(wide, &[UNASSIGNED]).is_err(),
        "the cached face is asked again, not handed back"
    );
    assert!(manager.is_unresolved(wide), "and the miss is recorded");
}

#[test]
fn an_unresolved_mark_can_be_cleared_by_changing_the_search() {
    let mut manager = FontManager::new();
    let id = manager.registry_mut().register_system(ABSENT);
    assert!(manager.face_for(id, &[UNASSIGNED]).is_err());
    assert!(manager.is_unresolved(id));
    manager.clear_unresolved(id);
    assert!(!manager.is_unresolved(id), "the search may run again");
}

#[test]
fn setting_the_pinned_family_clears_answers_that_depended_on_it() {
    let mut manager = FontManager::new();
    let id = manager.registry_mut().register_system(ABSENT);
    assert!(manager.face_for(id, &[UNASSIGNED]).is_err());
    assert!(manager.is_unresolved(id));
    manager.set_default_cjk_family("SimSun");
    assert!(
        !manager.is_unresolved(id),
        "a different search has to be allowed to run"
    );
}

#[test]
fn the_pinned_family_is_reported_and_settable() {
    let manager = FontManager::new();
    assert_eq!(manager.pinned_family(), "Noto Sans CJK SC");
    manager.set_default_cjk_family("SimSun");
    assert_eq!(manager.pinned_family(), "SimSun");
}

#[test]
fn a_list_of_ids_can_be_marked_unresolved_in_one_call() {
    let mut manager = FontManager::new();
    let id = manager.registry_mut().register_system(ABSENT);
    let ids: Vec<FontId> = vec![id];
    manager.mark_unresolved(&ids);
    assert!(manager.is_unresolved(id));
    assert_eq!(manager.unresolved().len(), 1, "and reported once");
}

#[test]
fn a_family_the_host_lacks_can_still_be_answered_by_a_configured_one() {
    // The failure this guards against is a search that reports success for a
    // face that draws nothing. The configured list names a family the host
    // does not have; what answers is the repo's own face, registered under it.
    let mut manager = FontManager::new();
    manager.set_config(FontConfig::new([ABSENT], std::iter::empty::<&str>()));
    let id = manager
        .registry_mut()
        .register_fallback_with_data(ABSENT, Some(cjk_fixture()));
    let face = manager
        .face_for(id, &[0x4E2D])
        .expect("the pinned bytes answer");
    assert!(
        face.has_codepoint(0x4E2D),
        "the substitute must draw the character it was found for"
    );
}

#[test]
fn a_latin_only_face_does_not_stand_in_for_a_han_run() {
    let mut manager = FontManager::new();
    let narrow = manager.registry_mut().register_embedded(
        "Res/latin.ttf",
        "Latin",
        std::sync::Arc::new(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/data/steleofd-latin-subset.ttf"
            ))
            .expect("fixture"),
        ),
    );
    assert!(manager.face_for(narrow, &[b'A' as u32]).is_ok());
    // An embedded face is the document's own: no substitute may replace it,
    // but the miss has to be visible rather than a blank glyph.
    assert!(manager.face_for(narrow, &[0x4E2D]).is_err());
    assert!(manager.is_unresolved(narrow));
}
