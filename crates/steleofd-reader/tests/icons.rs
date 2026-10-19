//! The icon set, drawn the way a frame draws it. The checks are that the stored
//! path data reads, that every glyph lands inside its grid, and that no two of
//! them are the same drawing.

use eframe::egui::epaint::{ClippedShape, Mesh, Primitive, Tessellator};
use eframe::egui::{self, Rect};
use steleofd_reader::icons::{self, Icon};

/// The edge a glyph is drawn at in these checks.
const BOX: f32 = 64.0;

/// The square a glyph is drawn on, as a frame gives it.
fn box_rect() -> Rect {
    Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(BOX, BOX))
}

/// Every shape a glyph produced when painted, and the mesh they tessellated to.
fn drawn(icon: Icon) -> (Vec<egui::Shape>, Mesh) {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(96.0, 96.0))
        .build_ui(|ui| {
            icons::paint(ui.painter(), box_rect(), icon, egui::Color32::WHITE);
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
    let mut mesh = Mesh::default();
    for primitive in tessellator.tessellate_shapes(clipped) {
        if let Primitive::Mesh(part) = primitive.primitive {
            let offset = mesh.vertices.len() as u32;
            mesh.vertices.extend(part.vertices);
            mesh.indices.extend(part.indices.iter().map(|i| i + offset));
        }
    }
    (shapes, mesh)
}

/// The bounding box of every point a glyph drew.
fn bounds(shapes: &[egui::Shape]) -> Option<Rect> {
    let mut rect: Option<Rect> = None;
    for shape in shapes {
        if let egui::Shape::Path(path) = shape {
            for point in &path.points {
                rect = Some(match rect {
                    None => Rect::from_min_max(*point, *point),
                    Some(current) => current.union(Rect::from_min_max(*point, *point)),
                });
            }
        }
    }
    rect
}

#[test]
fn every_icon_is_a_filled_drawing() {
    for icon in Icon::ALL {
        let (_, mesh) = drawn(icon);
        assert!(
            mesh.indices.len() >= 9,
            "{} tessellated to {} indices",
            icon.theme_name(),
            mesh.indices.len()
        );
    }
}

#[test]
fn every_icon_stays_in_its_grid() {
    // The glyphs are the theme's own, drawn on the 16-unit grid, so the whole
    // drawing sits inside it, with a point of slack for rounding.
    let allowed = box_rect().expand(1.0);
    for icon in Icon::ALL {
        let (shapes, _) = drawn(icon);
        let bounds =
            bounds(&shapes).unwrap_or_else(|| panic!("{} drew nothing", icon.theme_name()));
        assert!(
            allowed.contains_rect(bounds),
            "{} left its grid: {bounds:?}",
            icon.theme_name()
        );
    }
}

#[test]
fn every_icon_fills_a_share_of_its_grid() {
    // A glyph that collapsed to a dot would still pass the test above, so this
    // one asks for a drawing worth putting in a button: it spans a third of the
    // square either way, which is what a bar of three does.
    for icon in Icon::ALL {
        let (shapes, _) = drawn(icon);
        let bounds = bounds(&shapes).expect("drew something");
        assert!(
            bounds.width() > BOX * 0.3 && bounds.height() > BOX * 0.3,
            "{} is too small: {bounds:?}",
            icon.theme_name()
        );
    }
}

#[test]
fn no_two_icons_are_the_same_drawing() {
    for (index, icon) in Icon::ALL.iter().enumerate() {
        for other in &Icon::ALL[index + 1..] {
            let (mine, _) = drawn(*icon);
            let (theirs, _) = drawn(*other);
            assert_ne!(
                format!("{mine:?}"),
                format!("{theirs:?}"),
                "{} and {} are the same drawing",
                icon.theme_name(),
                other.theme_name()
            );
        }
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

#[test]
fn a_glyph_becomes_a_texture() {
    let ctx = egui::Context::default();
    for icon in Icon::ALL {
        let handle = icons::texture(&ctx, icon, egui::Color32::WHITE);
        let size = handle.size();
        assert!(
            size[0] >= 32 && size[1] >= 32,
            "{} made a {size:?} texture",
            icon.theme_name()
        );
    }
}
