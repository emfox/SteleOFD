//! The icon set, drawn the way a frame draws it: through the tessellator. That
//! triangles came out is the check that a glyph is really there; where they
//! landed is the check that it keeps to its grid.

use eframe::egui::epaint::{ClippedShape, Mesh, Tessellator};
use eframe::egui::{self, Rect};
use steleofd_reader::icons::{self, Icon};

/// The space an icon is drawn in, at four times its nominal size so a stroke is
/// several pixels wide and a bounding box means something.
const BOX: f32 = 64.0;

/// Every triangle the painter produced for one glyph, as one mesh.
fn meshes(icon: Icon) -> (Vec<Mesh>, Vec<egui::Shape>) {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(96.0, 96.0))
        .build_ui(|ui| {
            let rect = Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(BOX, BOX));
            icons::paint(ui.painter(), rect, icon, egui::Color32::WHITE);
        });
    harness.run();
    let shapes: Vec<egui::Shape> = harness
        .output()
        .shapes
        .iter()
        .map(|s| s.shape.clone())
        .collect();
    let mut tessellator = Tessellator::new(1.0, Default::default(), [16, 16], Vec::new());
    let clipped: Vec<ClippedShape> = shapes
        .iter()
        .cloned()
        .map(|shape| ClippedShape {
            clip_rect: Rect::EVERYTHING,
            shape,
        })
        .collect();
    let primitives = tessellator.tessellate_shapes(clipped);
    let mut meshes = Vec::new();
    for primitive in primitives {
        if let epaint_primitive::Primitive::Mesh(mesh) = primitive.primitive {
            meshes.push(mesh);
        }
    }
    (meshes, shapes)
}

mod epaint_primitive {
    pub use eframe::egui::epaint::Primitive;
}

/// The bounding box of every vertex a glyph produced.
fn bounds(meshes: &[Mesh]) -> Option<Rect> {
    let mut rect: Option<Rect> = None;
    for mesh in meshes {
        for vertex in &mesh.vertices {
            let point = vertex.pos;
            rect = Some(match rect {
                None => Rect::from_min_max(point, point),
                Some(current) => current.union(Rect::from_min_max(point, point)),
            });
        }
    }
    rect
}

/// How many triangles a glyph produced.
fn triangles(meshes: &[Mesh]) -> usize {
    meshes.iter().map(|mesh| mesh.indices.len() / 3).sum()
}

#[test]
fn every_icon_becomes_triangles() {
    for icon in Icon::ALL {
        let (meshes, _) = meshes(icon);
        let count = triangles(&meshes);
        assert!(count > 8, "{:?} tessellated to {count} triangles", icon);
    }
}

#[test]
fn every_icon_keeps_to_its_box() {
    // A stroke of 1.5 units on a 16-unit square is a twelfth of the box, so a
    // glyph may reach half a stroke past it and no further.
    let box_rect = Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(BOX, BOX));
    let slack = BOX * (icons::STROKE / icons::SIZE) + 1.0;
    let allowed = box_rect.expand(slack);
    for icon in Icon::ALL {
        let (meshes, _) = meshes(icon);
        let bounds = bounds(&meshes).unwrap_or_else(|| panic!("{:?} drew nothing", icon));
        assert!(
            allowed.contains_rect(bounds),
            "{:?} left its box: {bounds:?}",
            icon
        );
    }
}

#[test]
fn the_theme_names_are_distinct() {
    let mut names: Vec<&str> = Icon::ALL.iter().map(|icon| icon.theme_name()).collect();
    names.sort_unstable();
    let count = names.len();
    names.dedup();
    assert_eq!(count, names.len(), "two icons claim the same theme name");
}
