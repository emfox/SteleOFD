//! The render layer driven as a consumer.
//! A page is rasterized through `render_page` and inspected pixel by pixel, so
//! the tests assert what reaches the buffer, plus the budget and size guards.

use std::sync::Arc;

use steleofd_common::{Millimeter, Rect};
use steleofd_display::ops::{Color, Paint, PaintRule, StrokeStyle};
use steleofd_display::text::GlyphRun;
use steleofd_display::{DisplayListBuilder, DrawOp, ImageCodec, ImageRef, PageDisplay};
use steleofd_font::{FontId, FontManager};
use steleofd_geometry::{Affine, Path};
use steleofd_render::{RenderOptions, render_page};

fn rect(x: f64, y: f64, w: f64, h: f64) -> Path {
    Path::build(|b| {
        b.rect(x, y, w, h);
    })
}

fn page_with(ops: Vec<DrawOp>) -> PageDisplay {
    let mut builder = DisplayListBuilder::new(Rect::A4_PORTRAIT);
    for op in ops {
        builder.push(op);
    }
    builder.finish(0)
}

fn filled(path: Path, color: Color) -> DrawOp {
    DrawOp::Path {
        path,
        fill: Some(Paint::Solid(color)),
        stroke: None,
        stroke_style: StrokeStyle::default(),
        rule: PaintRule::NonZero,
    }
}

/// The RGBA pixel at (x, y) in a rasterization.
fn pixel(output: &steleofd_render::RasterOutput, x: u32, y: u32) -> [u8; 4] {
    let bytes = output.rgba_bytes();
    let base = ((y * output.width + x) * 4) as usize;
    [
        bytes[base],
        bytes[base + 1],
        bytes[base + 2],
        bytes[base + 3],
    ]
}

/// The fixture face, registered under id 0.
fn manager_with_fixture() -> (FontManager, FontId) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/Orbitron-Bold.ttf");
    let data = std::fs::read(path).expect("font fixture");
    let mut manager = FontManager::new();
    let id = manager.registry_mut().register_embedded(
        "Res/Orbitron-Bold.ttf",
        "Orbitron",
        Arc::new(data),
    );
    (manager, id)
}

#[test]
fn a_blank_page_rasterizes_to_the_requested_size() {
    let fonts = FontManager::new();
    let output =
        render_page(&page_with(Vec::new()), &fonts, &RenderOptions::default()).expect("render");
    // A4 at 96 DPI is about 794x1123 pixels.
    assert!(
        (output.width as i64 - 794).abs() <= 2,
        "width {}",
        output.width
    );
    assert!(
        (output.height as i64 - 1123).abs() <= 2,
        "height {}",
        output.height
    );
    // The default background is white, so a blank page is uniformly white.
    assert_eq!(pixel(&output, 10, 10), [255, 255, 255, 255]);
}

#[test]
fn the_background_colour_is_applied_before_the_content() {
    let fonts = FontManager::new();
    let options = RenderOptions {
        background: Some(Color::rgba(10, 20, 30, 255)),
        ..Default::default()
    };
    let output = render_page(&page_with(Vec::new()), &fonts, &options).expect("render");
    assert_eq!(pixel(&output, 5, 5), [10, 20, 30, 255]);
}

#[test]
fn a_transparent_background_leaves_the_page_clear() {
    let fonts = FontManager::new();
    let options = RenderOptions {
        background: None,
        ..Default::default()
    };
    let output = render_page(&page_with(Vec::new()), &fonts, &options).expect("render");
    assert_eq!(pixel(&output, 5, 5), [0, 0, 0, 0], "alpha must be zero");
}

#[test]
fn a_filled_rectangle_paints_the_pixels_it_covers() {
    let fonts = FontManager::new();
    let page = page_with(vec![filled(
        rect(10.0, 10.0, 50.0, 50.0),
        Color::rgba(255, 0, 0, 255),
    )]);
    let output = render_page(&page, &fonts, &RenderOptions::default()).expect("render");

    // 10mm at 96 DPI is about 38px, so (100, 100) is well inside the rectangle
    // and (600, 600) is well outside it.
    let inside = pixel(&output, 100, 100);
    assert_eq!(inside[0], 255, "red channel inside: {inside:?}");
    assert!(
        inside[1] < 60 && inside[2] < 60,
        "should be red, got {inside:?}"
    );

    // A point outside the rectangle stays white.
    assert_eq!(pixel(&output, 600, 600), [255, 255, 255, 255]);
}

#[test]
fn the_pixel_budget_is_enforced_before_the_raster_is_allocated() {
    let fonts = FontManager::new();
    let options = RenderOptions {
        // Far too small for an A4 page at 96 DPI.
        max_pixels: 1024,
        ..Default::default()
    };
    let err = render_page(&page_with(Vec::new()), &fonts, &options).expect_err("budget");
    assert!(err.to_string().contains("budget"), "{err}");
}

#[test]
fn a_higher_dpi_produces_a_larger_raster() {
    let fonts = FontManager::new();
    let page = page_with(Vec::new());
    let low = render_page(&page, &fonts, &RenderOptions::default()).expect("96");
    let high = render_page(
        &page,
        &fonts,
        &RenderOptions {
            dpi: 192.0,
            ..Default::default()
        },
    )
    .expect("192");
    assert!(high.width > low.width);
    assert!(high.height > low.height);
}

#[test]
fn an_unbalanced_restore_is_reported_rather_than_panicking() {
    let fonts = FontManager::new();
    // A hand-built page can carry a restore with no save; the renderer must
    // report it instead of indexing into an empty stack.
    let page = page_with(vec![DrawOp::Restore]);
    let err = render_page(&page, &fonts, &RenderOptions::default()).expect_err("unbalanced");
    assert!(err.to_string().contains("restore"), "{err}");
}

#[test]
fn a_transform_moves_the_painted_pixels() {
    let fonts = FontManager::new();
    // A small square at the origin, translated right by 100mm.
    let page = page_with(vec![
        DrawOp::Transform(Affine::translate(100.0, 100.0)),
        filled(rect(0.0, 0.0, 20.0, 20.0), Color::rgba(0, 0, 255, 255)),
    ]);
    let output = render_page(&page, &fonts, &RenderOptions::default()).expect("render");
    // The translated square covers around (100mm, 100mm), not the origin.
    let moved = pixel(&output, 400, 400);
    assert!(
        moved[2] > 200,
        "expected blue where the square moved, got {moved:?}"
    );
    assert_eq!(
        pixel(&output, 10, 10),
        [255, 255, 255, 255],
        "origin stays clear"
    );
}

#[test]
fn a_clip_confines_the_painted_pixels() {
    let fonts = FontManager::new();
    // A large red rectangle clipped to a small square at the origin.
    let page = page_with(vec![
        DrawOp::ClipPath {
            path: rect(0.0, 0.0, 20.0, 20.0),
            rule: PaintRule::NonZero,
        },
        filled(rect(0.0, 0.0, 200.0, 200.0), Color::rgba(255, 0, 0, 255)),
    ]);
    let output = render_page(&page, &fonts, &RenderOptions::default()).expect("render");
    // Inside the clip is painted; well outside it is not, even though the path
    // covered it.
    assert_eq!(pixel(&output, 30, 30)[0], 255);
    assert_eq!(pixel(&output, 300, 300), [255, 255, 255, 255]);
}

#[test]
fn a_clip_of_two_subpaths_paints_both_of_them() {
    let fonts = FontManager::new();
    // Clause 8.4: the areas of a clip cover their union, so a clip path made of
    // two separate squares leaves the gap between them unpainted and both
    // squares painted. Reading the areas as one each would lose half.
    let clip = Path::build(|b| {
        b.rect(0.0, 0.0, 20.0, 20.0);
        b.rect(60.0, 0.0, 20.0, 20.0);
    });
    let page = page_with(vec![
        DrawOp::ClipPath {
            path: clip,
            rule: PaintRule::NonZero,
        },
        filled(rect(0.0, 0.0, 200.0, 200.0), Color::rgba(255, 0, 0, 255)),
    ]);
    let output = render_page(&page, &fonts, &RenderOptions::default()).expect("render");
    assert_eq!(pixel(&output, 30, 30)[0], 255, "the first square");
    assert_eq!(pixel(&output, 190, 30)[0], 255, "the second square");
    assert_eq!(
        pixel(&output, 110, 30),
        [255, 255, 255, 255],
        "the gap between them is outside the union"
    );
}

#[test]
fn a_run_with_no_glyphs_paints_nothing() {
    let (fonts, id) = manager_with_fixture();
    // An empty run is a legitimate input — a blank line in a text block — and
    // must leave the page untouched rather than fail.
    let run = GlyphRun {
        font: id,
        size: Millimeter(20.0),
        glyphs: Vec::new(),
        advances: Vec::new(),
        text: Some(String::new()),
        cluster_map: None,
        codes_are_glyphs: false,
        transform: Affine::IDENTITY,
        fill: Some(Paint::Solid(Color::BLACK)),
        stroke: None,
        letter_spacing: 0.0,
        word_spacing: 0.0,
        bbox: Rect::new(0.0, 0.0, 0.0, 0.0),
    };
    let page = page_with(vec![DrawOp::Text(run)]);
    let output = render_page(&page, &fonts, &RenderOptions::default()).expect("render");
    assert_eq!(pixel(&output, 50, 50), [255, 255, 255, 255]);
}

#[test]
fn a_degenerate_page_box_still_yields_one_pixel() {
    let fonts = FontManager::new();
    // A zero-size box is degenerate but not fatal: the size helper floors at a
    // single pixel rather than producing a zero-dimension raster that no
    // encoder could write.
    let mut builder = DisplayListBuilder::new(Rect::new(0.0, 0.0, 0.0, 0.0));
    builder.push(filled(rect(0.0, 0.0, 1.0, 1.0), Color::BLACK));
    let page = builder.finish(0);
    let output = render_page(&page, &fonts, &RenderOptions::default()).expect("render");
    assert_eq!((output.width, output.height), (1, 1));
    assert_eq!(output.rgba_bytes().len(), 4, "one RGBA pixel");
}

/// The placement transform used to be composed the wrong way round, so a
/// picture landed thousands of pixels off the page and rendered as pure white
/// while the command still reported success.
#[test]
fn an_image_lands_inside_the_rect_its_boundary_names() {
    let fonts = FontManager::new();
    // A 4x4 red PNG, drawn into a 40x40 mm box 20 mm from each edge.
    let png = {
        let mut px = tiny_skia::Pixmap::new(4, 4).expect("pixmap");
        px.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));
        px.encode_png().expect("png")
    };
    let dst = Rect::new(20.0, 20.0, 40.0, 40.0);
    let image = ImageRef::new("1", Arc::new(png), (4, 4), ImageCodec::Png);
    let page = page_with(vec![DrawOp::Image {
        image,
        dst,
        transform: Affine::IDENTITY,
    }]);
    let options = RenderOptions::default();
    let output = render_page(&page, &fonts, &options).expect("render");
    let k = options.dpi / 25.4;
    let cx = ((dst.x + dst.w / 2.0) * k) as u32;
    let cy = ((dst.y + dst.h / 2.0) * k) as u32;
    assert_eq!(
        pixel(&output, cx, cy),
        [255, 0, 0, 255],
        "red at the box centre"
    );
    assert_eq!(
        pixel(&output, 2, 2),
        [255, 255, 255, 255],
        "the top-left corner stays outside the box"
    );
}

/// JBIG2 is the encoding a scanned page carries, and `image` does not read it,
/// so it takes a separate path through the decoder.
#[test]
fn a_jbig2_image_is_decoded_and_painted() {
    let fonts = FontManager::new();
    let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/qr.jb2"))
        .expect("the sample is present");
    let (w, h) = steleofd_display::jbig2::dimensions(&data).expect("dimensions");
    assert_eq!((w, h), (100, 100), "the sample is a 100x100 QR code");
    let dst = Rect::new(20.0, 20.0, 50.0, 50.0);
    let image = ImageRef::new("1", Arc::new(data), (w, h), ImageCodec::Jbig2);
    let page = page_with(vec![DrawOp::Image {
        image,
        dst,
        transform: Affine::IDENTITY,
    }]);
    let output = render_page(&page, &fonts, &RenderOptions::default()).expect("render");
    let bytes = output.rgba_bytes();
    let (pixels, _) = bytes.as_chunks::<4>();
    let dark = pixels.iter().filter(|p| p[0] < 128).count();
    assert!(
        dark > 1000,
        "the QR code paints {dark} dark pixels, not none"
    );
}

/// The decoded page is not a black rectangle: a QR code is mostly white with
/// dark modules, so a fully dark result would mean the bit order is inverted.
#[test]
fn a_jbig2_page_is_black_on_white_rather_than_inverted() {
    let _fonts = FontManager::new();
    let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/qr.jb2"))
        .expect("the sample is present");
    let gray = steleofd_display::jbig2::decode_to_gray(&data).expect("decode");
    assert_eq!(gray.len(), 100 * 100, "one byte per pixel");
    let black = gray.iter().filter(|&&v| v == 0).count();
    let ratio = black as f64 / gray.len() as f64;
    assert!(
        (0.2..0.6).contains(&ratio),
        "black covers {ratio:.2} of the page, which is neither blank nor solid"
    );
}

/// A resource the archive never supplied leaves the area empty rather than
/// failing the page, which is what a missing file has always meant.
#[test]
fn a_placement_with_no_bytes_leaves_the_area_untouched() {
    let fonts = FontManager::new();
    let image = ImageRef::new("1", Arc::new(Vec::new()), (1, 1), ImageCodec::Png);
    let page = page_with(vec![DrawOp::Image {
        image,
        dst: Rect::new(10.0, 10.0, 50.0, 50.0),
        transform: Affine::IDENTITY,
    }]);
    let output = render_page(&page, &fonts, &RenderOptions::default()).expect("render");
    assert_eq!(pixel(&output, 100, 100), [255, 255, 255, 255]);
}

/// GIF is an encoding GB/T 33190's own test documents embed, so the decoder
/// has to read it rather than fail the page that places it.
#[test]
fn a_gif_image_is_decoded_and_painted() {
    let fonts = FontManager::new();
    let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/block.gif"))
        .expect("the sample is present");
    let dst = Rect::new(20.0, 20.0, 40.0, 30.0);
    let image = ImageRef::new("1", Arc::new(data), (40, 30), ImageCodec::Gif);
    let page = page_with(vec![DrawOp::Image {
        image,
        dst,
        transform: Affine::IDENTITY,
    }]);
    let output = render_page(&page, &fonts, &RenderOptions::default()).expect("render");
    let bytes = output.rgba_bytes();
    let (pixels, _) = bytes.as_chunks::<4>();
    let dark = pixels.iter().filter(|p| p[0] < 128).count();
    let light = pixels.iter().filter(|p| p[0] > 200).count();
    assert!(
        dark > 100 && light > 100,
        "the sample's black block and white ground both paint, got {dark} dark and {light} light"
    );
}
