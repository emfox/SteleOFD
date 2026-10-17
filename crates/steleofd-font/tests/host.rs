//! A CJK substitute on a host that does not ship the family the engine pins:
//! Windows and macOS keep their ideographs elsewhere, and font-kit maps the
//! generic names to Latin faces, so a host with no Noto face must still find one.

use steleofd_font::{FontId, FontManager};

/// A family name no host installs, used where the point is a missing face.
const ABSENT: &str = "No Such Family 98765";

#[test]
fn a_loaded_substitute_is_searched_again_for_characters_it_lacks() {
    // A face is cached for an id once any caller accepts it, so a run of Han
    // after a run of Latin would reuse the Latin face and drop every mark.
    let mut manager = FontManager::new();
    let wide = manager
        .registry_mut()
        .register_fallback_with_data("cjk-pinned", None);
    assert!(manager.face_for(wide, &[b'A' as u32]).is_ok());
    // Whether the host has a Han face decides the outcome; either way the
    // cached single-face answer must not be handed back unchanged.
    let _ = manager.face_for(wide, &[0x4E2D]);
    assert!(
        manager.cjk_answered().is_some() || manager.is_unresolved(wide),
        "a Han run is either answered by a new face or reported, never ignored"
    );
}

#[test]
fn an_unresolved_mark_can_be_cleared_by_changing_the_search() {
    let mut manager = FontManager::new();
    let id = manager.registry_mut().register_system(ABSENT);
    assert!(manager.face_for(id, &[0x10_FF_FD]).is_err());
    assert!(manager.is_unresolved(id));
    manager.clear_unresolved(id);
    assert!(!manager.is_unresolved(id), "the search may run again");
}

#[test]
fn setting_the_pinned_family_clears_answers_that_depended_on_it() {
    let mut manager = FontManager::new();
    let id = manager.registry_mut().register_system(ABSENT);
    assert!(manager.face_for(id, &[0x10_FF_FD]).is_err());
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
fn a_host_without_the_pinned_family_still_finds_chinese() {
    // The pinned family is what a Linux desktop usually has and Windows and
    // macOS usually do not, so this names one this machine also lacks and
    // asserts only that a Han run is answered by something that draws it.
    let mut manager = FontManager::new();
    manager.set_default_cjk_family("Microsoft YaHei");
    let wide = manager
        .registry_mut()
        .register_fallback_with_data("cjk-pinned", None);
    let Ok(face) = manager.face_for(wide, &[0x4E2D]) else {
        // No CJK face on this host at all: the miss has to be reported.
        assert!(manager.is_unresolved(wide));
        return;
    };
    assert!(
        face.has_codepoint(0x4E2D),
        "the substitute must draw the character it was found for"
    );
    assert!(
        manager.cjk_answered().is_some(),
        "and the family that answered is remembered"
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
