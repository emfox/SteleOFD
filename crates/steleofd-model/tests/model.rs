//! The model driven as a consumer, asserting the assembly invariants.
//! A model is built here the way the parser builds it, then queried through the
//! public accessors, so fields and accessors that disagree would fail.

use steleofd_common::Rect;
use steleofd_error::OfdError;
use steleofd_model::document::{Layer, PageArea};
use steleofd_model::object::{ObjectBase, PathObject, TextCode, TextObject};
use steleofd_model::{
    CommonData, CompositeGraphicUnit, DocBody, Document, Location, Multimedia, MultimediaKind,
    Object, OfdRoot, Page, PageBlock, PageLayout, PageMode, Pages, Permission, PrintPermission,
    TabDisplay, VPreferences, Valid, ValidPeriod, Version, ZoomMode, ZoomSetting,
};

fn path_object(id: &str) -> Object {
    Object::PathObject(PathObject {
        base: ObjectBase {
            id: Some(id.to_string()),
            boundary: Rect::new(0.0, 0.0, 10.0, 10.0),
            ..Default::default()
        },
        ..Default::default()
    })
}

fn text_object(id: &str, text: &str) -> Object {
    Object::TextObject(TextObject {
        base: ObjectBase {
            id: Some(id.to_string()),
            boundary: Rect::new(0.0, 0.0, 50.0, 10.0),
            ..Default::default()
        },
        codes: vec![TextCode {
            x: 0.0,
            y: 0.0,
            text: text.to_string(),
            ..Default::default()
        }],
        ..Default::default()
    })
}

fn layer_of(objects: Vec<Object>) -> Layer {
    Layer {
        id: Some("1".to_string()),
        objects,
    }
}

/// Pull the text-bearing object out of an `Object` so its methods can be
/// checked on the concrete type, which is where they live.
fn as_text(object: &Object) -> &TextObject {
    match object {
        Object::TextObject(t) => t,
        other => panic!("not a text object: {other:?}"),
    }
}

fn page_with(objects: Vec<Object>) -> Page {
    Page {
        id: "1".to_string(),
        content: Some(PageBlock {
            layers: vec![layer_of(objects)],
        }),
        ..Default::default()
    }
}

#[test]
fn a_root_without_a_document_body_is_not_an_ofd() {
    let root = OfdRoot::default();
    let err = root.validate().expect_err("empty root");
    assert!(matches!(err, OfdError::Format(_)), "{err:?}");
}

#[test]
fn a_document_body_without_a_root_path_is_rejected() {
    let root = OfdRoot {
        version: Version::default(),
        doc_bodies: vec![DocBody {
            doc_root: Location::new(""),
            ..Default::default()
        }],
    };
    assert!(
        root.validate().is_err(),
        "an empty DocRoot must not validate"
    );
}

#[test]
fn a_complete_root_validates_and_exposes_its_primary_body() {
    let root = OfdRoot {
        version: Version::default(),
        doc_bodies: vec![DocBody {
            doc_root: Location::new("Doc_0/Document.xml"),
            ..Default::default()
        }],
    };
    root.validate().expect("valid root");
    assert_eq!(
        root.primary_body().expect("body").doc_root.value,
        "Doc_0/Document.xml"
    );
}

#[test]
fn a_root_with_no_body_reports_that_rather_than_indexing() {
    let root = OfdRoot::default();
    let err = root.primary_body().expect_err("no body");
    assert!(err.to_string().contains("DocBody"), "{err}");
}

#[test]
fn the_default_version_is_the_standard_major() {
    let version = Version::default();
    assert_eq!(version.major, "1");
    assert_eq!(version.sub, None);
}

#[test]
fn a_page_list_reports_its_length_and_rejects_an_out_of_range_index() {
    let pages = Pages {
        pages: vec![Page::default(), Page::default()],
    };
    assert_eq!(pages.len(), 2);
    assert!(!pages.is_empty());
    assert!(pages.get(1).is_ok());

    let err = pages.get(2).expect_err("out of range");
    let msg = err.to_string();
    assert!(msg.contains('2'), "{msg}");
}

#[test]
fn an_empty_page_list_is_empty_and_out_of_range_for_any_index() {
    let pages = Pages::default();
    assert!(pages.is_empty());
    assert_eq!(pages.len(), 0);
    assert!(pages.get(0).is_err());
}

#[test]
fn a_page_block_counts_the_objects_across_its_layers() {
    let block = PageBlock {
        layers: vec![
            layer_of(vec![path_object("1"), text_object("2", "hi")]),
            layer_of(vec![path_object("3")]),
        ],
    };
    assert!(block.has_content());
    assert_eq!(block.object_count(), 3);
}

#[test]
fn an_empty_page_block_has_no_content() {
    let block = PageBlock {
        layers: vec![layer_of(Vec::new())],
    };
    // A layer with no objects is still no content: the page would draw blank.
    assert!(!block.has_content());
    assert_eq!(block.object_count(), 0);
}

#[test]
fn a_page_that_delegates_to_a_template_has_no_content_of_its_own() {
    let page = Page {
        id: "1".to_string(),
        content: None,
        ..Default::default()
    };
    assert!(!page.has_own_content());
}

#[test]
fn a_page_with_objects_reports_its_own_content() {
    let page = page_with(vec![path_object("1")]);
    assert!(page.has_own_content());
}

#[test]
fn an_object_reports_the_id_it_was_built_with() {
    let object = path_object("42");
    assert_eq!(object.id().map(String::as_str), Some("42"));
}

#[test]
fn an_unknown_object_is_not_renderable_and_carries_no_id() {
    let object = Object::unknown();
    assert!(!object.is_renderable());
    assert_eq!(object.id(), None);
    // Every object this build understands is renderable, which is the property
    // the flattening pass relies on to decide what to skip.
    assert!(path_object("1").is_renderable());
    assert!(text_object("2", "x").is_renderable());
}

#[test]
fn a_text_object_reports_whether_it_carries_any_text() {
    assert!(as_text(&text_object("1", "hello")).has_text());
    assert!(!as_text(&text_object("2", "")).has_text());
}

#[test]
fn common_data_round_trips_a_page_area_into_the_document() {
    let document = Document {
        common_data: CommonData {
            page_area: PageArea::new(Rect::A4_PORTRAIT),
            ..Default::default()
        },
        pages: Pages {
            pages: vec![page_with(vec![path_object("1")])],
        },
        vpreferences: None,
        permissions: None,
    };
    assert_eq!(
        document.common_data.page_area.physical_box,
        Rect::A4_PORTRAIT
    );
    assert_eq!(document.pages.len(), 1);
}

#[test]
fn a_location_builds_from_a_string_or_a_path() {
    assert_eq!(Location::new("a/b.xml").value, "a/b.xml");
    let from_ref: Location = "c/d.xml".into();
    assert_eq!(from_ref.value, "c/d.xml");
}

#[test]
fn a_multimedia_resource_keeps_the_kind_it_was_parsed_with() {
    let video = Multimedia {
        id: "9".to_string(),
        kind: MultimediaKind::parse("Video"),
        format: Some("mp4".to_string()),
        media_file: "Res/clip.mp4".to_string(),
    };
    assert_eq!(video.kind, MultimediaKind::Video);
    assert_eq!(video.kind.as_str(), "Video");

    // An unknown spelling degrades to the default rather than failing, because
    // a new media kind must not make an older reader reject the document.
    assert_eq!(MultimediaKind::parse("Hologram"), MultimediaKind::default());
}

#[test]
fn a_composite_graphic_unit_keeps_its_declared_size() {
    let unit = CompositeGraphicUnit {
        id: "5".to_string(),
        width: 80.0,
        height: 40.0,
        ..Default::default()
    };
    assert_eq!(unit.width, 80.0);
    assert_eq!(unit.height, 40.0);
    assert!(unit.content.is_none());
}

#[test]
fn viewer_preferences_round_trip_in_the_shape_the_schema_uses() {
    // Clause 7.11's element names are what a writer has to emit, so the
    // serialised form matters as much as the parsed one.
    let prefs = VPreferences {
        page_mode: Some(PageMode::UseBookmarks),
        page_layout: Some(PageLayout::TwoColumnR),
        tab_display: Some(TabDisplay::DocTitle),
        hide_toolbar: Some(true),
        hide_menubar: None,
        hide_window_ui: Some(false),
        zoom: ZoomSetting::ZoomMode(ZoomMode::FitHeight),
    };
    let json = serde_json::to_value(&prefs).expect("serialize");
    assert_eq!(json["PageMode"], "UseBookmarks");
    assert_eq!(json["PageLayout"], "TwoColumnR");
    assert_eq!(json["TabDisplay"], "DocTitle");
    assert_eq!(json["HideToolbar"], true);
    assert_eq!(json["HideWindowUI"], false);
    assert_eq!(json["ZoomMode"], "FitHeight");
    assert!(
        json.get("HideMenubar").is_none(),
        "an unstated preference is left out"
    );

    let back: VPreferences = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, prefs);
}

#[test]
fn a_zoom_factor_round_trips_as_the_zoom_element() {
    let prefs = VPreferences {
        zoom: ZoomSetting::Zoom(2.5),
        ..Default::default()
    };
    let json = serde_json::to_value(&prefs).expect("serialize");
    assert_eq!(json["Zoom"], 2.5);
    assert!(json.get("ZoomMode").is_none(), "the two are exclusive");
    let back: VPreferences = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back.zoom, ZoomSetting::Zoom(2.5));
}

#[test]
fn an_unset_zoom_adds_no_element() {
    let json = serde_json::to_value(VPreferences::default()).expect("serialize");
    assert!(json.get("Zoom").is_none());
    assert!(json.get("ZoomMode").is_none());
}

#[test]
fn permissions_round_trip_in_the_shape_the_schema_uses() {
    // Clause 7.12: `Print` and `ValidPeriod` carry their payload as attributes,
    // so the serialised form has to match or a writer would emit the wrong XML.
    let perms = Permission {
        edit: Some(false),
        annotate: Some(true),
        export: None,
        signature: None,
        watermark: Some(false),
        print_screen: Some(true),
        print: Some(PrintPermission {
            printable: true,
            copies: 3,
        }),
        valid_period: Some(ValidPeriod {
            start_date: Some("2024-01-01T00:00:00Z".to_string()),
            end_date: None,
        }),
    };
    let json = serde_json::to_value(&perms).expect("serialize");
    assert_eq!(json["Edit"], false);
    assert_eq!(json["Annot"], true);
    assert_eq!(json["Print"]["@Printable"], true);
    assert_eq!(json["Print"]["@Copies"], 3);
    assert_eq!(json["ValidPeriod"]["@StartDate"], "2024-01-01T00:00:00Z");
    assert!(
        json["ValidPeriod"].get("@EndDate").is_none(),
        "an unstated end date is left out"
    );

    let back: Permission = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, perms);
}

#[test]
fn an_absent_print_copy_count_reads_as_no_limit() {
    // `Copies` defaults to -1 on the wire; deserialising a `Print` that omits
    // it must land on that default rather than zero.
    let print: PrintPermission =
        serde_json::from_value(serde_json::json!({ "@Printable": true })).expect("deserialize");
    assert_eq!(print.copies, -1);
}
