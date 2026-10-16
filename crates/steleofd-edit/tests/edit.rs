//! Tests for command-pattern editing, undo and permission handling.

use std::sync::Arc;

use steleofd_common::Rect;
use steleofd_edit::{
    AddObject, AllowAll, DenyPages, EditOptions, EditSession, RemoveObject, SetPageArea,
};
use steleofd_error::{EditError, OfdError};
use steleofd_model::document::{CommonData, Document, Layer, Page, PageArea, PageBlock, Pages};
use steleofd_model::object::{Object, ObjectBase, PathObject};

fn path_object(id: &str) -> Object {
    Object::PathObject(PathObject {
        base: ObjectBase {
            id: Some(id.to_string()),
            ..ObjectBase::default()
        },
        ..PathObject::default()
    })
}

fn document_with(one_object: bool) -> Document {
    let mut layer = Layer::default();
    if one_object {
        layer.objects.push(path_object("obj1"));
    }
    Document {
        common_data: CommonData {
            page_area: PageArea {
                physical_box: Rect::new(0.0, 0.0, 210.0, 297.0),
                application_box: None,
            },
            templates: None,
            draw_params: None,
        },
        pages: Pages {
            pages: vec![Page {
                id: "1".into(),
                base_loc: None,
                area: None,
                content: Some(PageBlock {
                    layers: vec![layer],
                }),
                templates: None,
            }],
        },
        vpreferences: None,
        permissions: None,
    }
}

fn session(document: Document) -> EditSession {
    EditSession::new(document, EditOptions::default())
}

fn object_count(session: &EditSession) -> usize {
    session.document().pages.pages[0]
        .content
        .as_ref()
        .map(|c| c.layers[0].objects.len())
        .unwrap_or(0)
}

#[test]
fn add_object_appends_and_undo_removes_it() {
    let mut session = session(document_with(true));
    session
        .apply(AddObject {
            page_index: 0,
            layer_index: 0,
            object: path_object("obj2"),
            inserted_at: None,
        })
        .expect("apply");
    assert_eq!(object_count(&session), 2);
    session.undo().expect("undo");
    assert_eq!(object_count(&session), 1);
    session.redo().expect("redo");
    assert_eq!(object_count(&session), 2);
}

#[test]
fn remove_object_restores_on_undo() {
    let mut session = session(document_with(true));
    session
        .apply(RemoveObject {
            page_index: 0,
            layer_index: 0,
            object_index: 0,
            removed: None,
        })
        .expect("apply");
    assert_eq!(object_count(&session), 0);
    session.undo().expect("undo");
    assert_eq!(object_count(&session), 1);
    assert_eq!(
        session.document().pages.pages[0]
            .content
            .as_ref()
            .expect("content")
            .layers[0]
            .objects[0]
            .id(),
        Some(&"obj1".to_string())
    );
}

#[test]
fn set_page_area_round_trips() {
    let mut session = session(document_with(false));
    let area = PageArea {
        physical_box: Rect::new(0.0, 0.0, 148.0, 210.0),
        application_box: None,
    };
    session
        .apply(SetPageArea {
            page_index: 0,
            area,
            previous: None,
        })
        .expect("apply");
    assert_eq!(
        session.document().pages.pages[0]
            .area
            .as_ref()
            .expect("area")
            .physical_box,
        Rect::new(0.0, 0.0, 148.0, 210.0)
    );
    session.undo().expect("undo");
    assert!(
        session.document().pages.pages[0].area.is_none(),
        "undo clears the override"
    );
}

#[test]
fn missing_targets_are_reported() {
    let mut session = session(document_with(true));
    let out = session.apply(AddObject {
        page_index: 7,
        layer_index: 0,
        object: path_object("x"),
        inserted_at: None,
    });
    assert!(matches!(
        out,
        Err(OfdError::Edit(EditError::TargetNotFound(_)))
    ));
    let out = session.apply(AddObject {
        page_index: 0,
        layer_index: 3,
        object: path_object("x"),
        inserted_at: None,
    });
    assert!(matches!(
        out,
        Err(OfdError::Edit(EditError::TargetNotFound(_)))
    ));
    let out = session.apply(RemoveObject {
        page_index: 0,
        layer_index: 0,
        object_index: 9,
        removed: None,
    });
    assert!(matches!(
        out,
        Err(OfdError::Edit(EditError::TargetNotFound(_)))
    ));
    assert_eq!(session.history_len(), 0, "failed commands are not recorded");
}

#[test]
fn empty_history_reports_nothing_to_undo() {
    let mut session = session(document_with(true));
    assert!(!session.can_undo());
    assert!(!session.can_redo());
    assert!(matches!(
        session.undo(),
        Err(OfdError::Edit(EditError::NothingToUndo))
    ));
}

#[test]
fn redo_branch_is_dropped_after_a_new_command() {
    let mut session = session(document_with(true));
    let command = || AddObject {
        page_index: 0,
        layer_index: 0,
        object: path_object("obj"),
        inserted_at: None,
    };
    session.apply(command()).expect("first");
    session.undo().expect("undo");
    session.apply(command()).expect("second");
    assert!(!session.can_redo(), "the undone branch is discarded");
    assert_eq!(session.history_len(), 1);
}

#[test]
fn signature_guard_requires_confirmation() {
    let options = EditOptions {
        signature_count: 2,
        ..EditOptions::default()
    };
    let mut session = EditSession::new(document_with(true), options);
    let out = session.apply(AddObject {
        page_index: 0,
        layer_index: 0,
        object: path_object("x"),
        inserted_at: None,
    });
    match out {
        Err(OfdError::Edit(EditError::WouldInvalidateSignature { count })) => {
            assert_eq!(count, 2);
        }
        other => panic!("expected signature guard, got {other:?}"),
    }
    let allowed = EditOptions {
        signature_count: 2,
        allow_signature_invalidation: true,
        ..EditOptions::default()
    };
    let mut session = EditSession::new(document_with(true), allowed);
    session
        .apply(AddObject {
            page_index: 0,
            layer_index: 0,
            object: path_object("x"),
            inserted_at: None,
        })
        .expect("explicit confirmation allows the edit");
}

#[test]
fn permission_policy_can_deny_a_page() {
    let options = EditOptions {
        permissions: Some(Arc::new(DenyPages::new([0]))),
        ..EditOptions::default()
    };
    let mut session = EditSession::new(document_with(true), options);
    let out = session.apply(AddObject {
        page_index: 0,
        layer_index: 0,
        object: path_object("x"),
        inserted_at: None,
    });
    assert!(matches!(
        out,
        Err(OfdError::Edit(EditError::PermissionDenied(_)))
    ));
    let allowed = EditOptions {
        permissions: Some(Arc::new(AllowAll)),
        ..EditOptions::default()
    };
    let mut session = EditSession::new(document_with(true), allowed);
    session
        .apply(AddObject {
            page_index: 0,
            layer_index: 0,
            object: path_object("x"),
            inserted_at: None,
        })
        .expect("allowed");
}

#[test]
fn history_is_capped() {
    let options = EditOptions {
        max_history: 2,
        ..EditOptions::default()
    };
    let mut session = EditSession::new(document_with(false), options);
    for i in 0..5 {
        session
            .apply(AddObject {
                page_index: 0,
                layer_index: 0,
                object: path_object(&format!("obj{i}")),
                inserted_at: None,
            })
            .expect("apply");
    }
    assert_eq!(session.history_len(), 2);
    assert_eq!(object_count(&session), 5, "objects stay on the page");
}
