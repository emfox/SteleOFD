//! The family list as data: what the engine ships, what a document may say,
//! and what a caller may change. No test here draws a glyph, so they run the
//! same way on a host with no CJK face at all.

use sofd_font::{FamilyList, FontConfig, FontManager, default_cjk_families};

#[test]
fn the_shipped_list_is_a_list_of_names_in_order() {
    let families = default_cjk_families();
    assert!(!families.is_empty());
    assert!(families.iter().all(|name| !name.trim().is_empty()));
    assert_eq!(families[0], "Noto Sans CJK SC", "the pinned family leads");
}

#[test]
fn a_list_drops_blanks_and_repeats_whatever_the_case() {
    let list = FamilyList::new(["SimSun", "", "  ", "simsun", "Microsoft YaHei"]);
    assert_eq!(list.as_ref(), ["SimSun", "Microsoft YaHei"]);
}

#[test]
fn a_default_configuration_is_the_shipped_list() {
    let config = FontConfig::default();
    assert!(config.uses_default_cjk_families());
    assert_eq!(config.search_order(), default_cjk_families());
}

#[test]
fn a_document_list_leads_but_the_shipped_names_stay_behind_it() {
    // A document cannot know which faces this host installs, so its own list
    // is a preference, not a replacement.
    let config = FontConfig::new(["My Serif CJK"], std::iter::empty::<&str>());
    assert!(!config.uses_default_cjk_families());
    let order = config.search_order();
    assert_eq!(order[0], "My Serif CJK");
    assert!(order.iter().any(|name| name == "Microsoft YaHei"));
}

#[test]
fn the_fallback_families_come_last() {
    let config = FontConfig::new(["CJK Face"], ["Late Resort"]);
    let order = config.search_order();
    assert_eq!(order[0], "CJK Face");
    assert_eq!(order.last().map(String::as_str), Some("Late Resort"));
}

#[test]
fn a_configuration_round_trips_through_json() {
    let config = FontConfig::new(["CJK Face"], ["Late Resort"]);
    let text = serde_json::to_string(&config).expect("serializes");
    let back: FontConfig = serde_json::from_str(&text).expect("reads back");
    assert_eq!(back, config);
    // An empty configuration must not carry empty keys into a file.
    let bare = serde_json::to_string(&FontConfig::default()).expect("serializes");
    assert_eq!(bare, "{}");
}

#[test]
fn a_manager_takes_its_families_from_the_configuration() {
    let manager = FontManager::new();
    assert_eq!(manager.pinned_family(), "Noto Sans CJK SC");
    manager.set_config(FontConfig::new(["My CJK Face"], std::iter::empty::<&str>()));
    assert_eq!(manager.pinned_family(), "My CJK Face");
    assert_eq!(manager.config().search_order()[0], "My CJK Face");
}

#[test]
fn naming_one_family_first_keeps_the_rest_of_the_list() {
    let manager = FontManager::new();
    manager.set_default_cjk_family("SimSun");
    let order = manager.config().search_order();
    assert_eq!(order[0], "SimSun");
    assert!(
        order.iter().any(|name| name == "Microsoft YaHei"),
        "the shipped list is kept behind the named family"
    );
}

#[test]
fn a_list_can_be_extended_at_the_front_without_duplicating() {
    let list = FamilyList::new(["A", "B"]);
    let next = list.prepend("B");
    assert_eq!(next.as_ref(), ["B", "A"]);
}

#[test]
fn a_document_layer_fills_what_it_left_out_from_the_layer_below() {
    let user = FontConfig::new(["My CJK Face"], ["My Late Resort"]);
    let document = FontConfig::new(std::iter::empty::<&str>(), ["Document Resource"]);
    let merged = document.with_fallback_from(&user);
    assert_eq!(merged.cjk_families.as_ref(), ["My CJK Face"]);
    assert_eq!(
        merged.fallback_families.as_ref(),
        ["Document Resource"],
        "what the document said wins"
    );
}
