//! End-to-end coverage of the print pipeline: selection, placement, raster.

use sofd_common::Rect;
use sofd_display::ops::{Color, Paint, PaintRule, StrokeStyle};
use sofd_display::{DisplayList, DrawOp, PageDisplay};
use sofd_error::{OfdError, PrintError};
use sofd_font::FontManager;
use sofd_geometry::Path;
use sofd_print::{
    ColorMode, Duplex, Margins, Orientation, PaperSize, PrintSettings, ScaleMode, plan, render_job,
    render_planned,
};

fn page(index: usize, w: f64, h: f64) -> PageDisplay {
    let path = Path::build(|b| {
        b.rect(5.0, 5.0, 10.0, 10.0);
    });
    PageDisplay {
        index,
        page_box: Rect::new(0.0, 0.0, w, h),
        ops: vec![DrawOp::Path {
            path,
            fill: Some(Paint::Solid(Color::rgba(255, 0, 0, 255))),
            stroke: None,
            stroke_style: StrokeStyle::default(),
            rule: PaintRule::NonZero,
        }],
        content_bbox: Rect::EMPTY,
    }
}

fn list(pages: Vec<PageDisplay>) -> DisplayList {
    DisplayList {
        page_count: pages.len(),
        pages,
    }
}

fn pixel(raster: &sofd_print::RasterPage, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * raster.width + x) * 4) as usize;
    [
        raster.rgba[i],
        raster.rgba[i + 1],
        raster.rgba[i + 2],
        raster.rgba[i + 3],
    ]
}

#[test]
fn selection_strings_cover_the_useful_shapes() {
    assert_eq!(
        sofd_print::PageSelection::parse("all").expect("all"),
        sofd_print::PageSelection::All
    );
    assert_eq!(
        sofd_print::PageSelection::parse("3").expect("one"),
        sofd_print::PageSelection::List(vec![3])
    );
    assert_eq!(
        sofd_print::PageSelection::parse("2-4").expect("range"),
        sofd_print::PageSelection::List(vec![2, 3, 4])
    );
    assert_eq!(
        sofd_print::PageSelection::parse("1,3-4").expect("mixed"),
        sofd_print::PageSelection::List(vec![1, 3, 4])
    );
    assert_eq!(
        sofd_print::PageSelection::parse("  ").expect("blank"),
        sofd_print::PageSelection::All
    );
}

#[test]
fn broken_selection_strings_are_rejected() {
    let err = sofd_print::PageSelection::parse("1,x").expect_err("not a number");
    assert!(matches!(
        err,
        OfdError::Print(PrintError::InvalidSelection { .. })
    ));
    let err = sofd_print::PageSelection::parse("5-2").expect_err("backwards");
    assert!(matches!(
        err,
        OfdError::Print(PrintError::InvalidRange { .. })
    ));
    let err = sofd_print::PageSelection::parse("0").expect_err("pages count from one");
    assert!(matches!(
        err,
        OfdError::Print(PrintError::InvalidSelection { .. })
    ));
}

#[test]
fn resolving_dedups_and_sorts_the_selection() {
    let selection = sofd_print::PageSelection::List(vec![3, 1, 1]);
    assert_eq!(selection.resolve(3).expect("resolve"), vec![0, 2]);
}

#[test]
fn a_selection_beyond_the_document_is_rejected() {
    let selection = sofd_print::PageSelection::List(vec![4]);
    let err = selection.resolve(3).expect_err("out of range");
    assert!(matches!(
        err,
        OfdError::Print(PrintError::PageOutOfRange { page: 4, count: 3 })
    ));
}

#[test]
fn the_document_paper_follows_the_page_box() {
    let settings = PrintSettings {
        paper: PaperSize::Document,
        ..PrintSettings::default()
    };
    let job = plan(&list(vec![page(0, 148.0, 210.0)]), &settings).expect("plan");
    assert_eq!(job.pages[0].paper.w, 148.0);
    assert_eq!(job.pages[0].paper.h, 210.0);
}

#[test]
fn named_papers_have_their_iso_size() {
    assert_eq!(PaperSize::A4.size_mm(Rect::EMPTY), (210.0, 297.0));
    assert_eq!(PaperSize::B5.size_mm(Rect::EMPTY), (176.0, 250.0));
    assert_eq!(PaperSize::Letter.size_mm(Rect::EMPTY), (215.9, 279.4));
}

#[test]
fn a_custom_scale_shrinks_the_content_onto_the_sheet() {
    let settings = PrintSettings {
        paper: PaperSize::Document,
        margins: Margins::none(),
        scale: ScaleMode::Custom(50.0),
        ..PrintSettings::default()
    };
    let job = plan(&list(vec![page(0, 20.0, 20.0)]), &settings).expect("plan");
    let top_left = job.pages[0].placement.apply(0.0, 0.0);
    let bottom_right = job.pages[0].placement.apply(20.0, 20.0);
    // Half scale centres a 10mm square on the 20mm sheet.
    assert!((top_left.0 - 5.0).abs() < 1e-9);
    assert!((bottom_right.0 - 15.0).abs() < 1e-9);
}

#[test]
fn a_portrait_page_on_a_portrait_sheet_is_not_rotated() {
    let settings = PrintSettings {
        paper: PaperSize::A4,
        ..PrintSettings::default()
    };
    let job = plan(&list(vec![page(0, 210.0, 297.0)]), &settings).expect("plan");
    let placement = job.pages[0].placement;
    assert!(placement.b.abs() < 1e-9 && placement.c.abs() < 1e-9);
}

#[test]
fn a_rendered_sheet_has_the_paper_size_at_the_job_dpi() {
    let settings = PrintSettings {
        paper: PaperSize::Document,
        margins: Margins::none(),
        dpi: 96.0,
        ..PrintSettings::default()
    };
    let pages = list(vec![page(0, 20.0, 20.0)]);
    let job = plan(&pages, &settings).expect("plan");
    let fonts = FontManager::new();
    let raster = render_planned(&pages.pages[0], &fonts, &job.pages[0], &job).expect("render");
    // 20 mm at 96 DPI is 75.6 pixels, rounded up.
    assert_eq!(raster.width, 76);
    assert_eq!(raster.height, 76);
    assert_eq!(raster.dpi, 96.0);
}

#[test]
fn content_lands_where_the_placement_puts_it() {
    let settings = PrintSettings {
        paper: PaperSize::Document,
        margins: Margins::none(),
        dpi: 96.0,
        ..PrintSettings::default()
    };
    let pages = list(vec![page(0, 20.0, 20.0)]);
    let job = plan(&pages, &settings).expect("plan");
    let fonts = FontManager::new();
    let raster = render_planned(&pages.pages[0], &fonts, &job.pages[0], &job).expect("render");
    assert_eq!(pixel(&raster, 38, 38)[0], 255, "inside the rect is red");
    assert_eq!(
        pixel(&raster, 4, 4),
        [255, 255, 255, 255],
        "the margin stays white"
    );
}

#[test]
fn monochrome_output_collapses_the_channels() {
    let settings = PrintSettings {
        paper: PaperSize::Document,
        margins: Margins::none(),
        dpi: 96.0,
        color: ColorMode::Mono,
        ..PrintSettings::default()
    };
    let pages = list(vec![page(0, 20.0, 20.0)]);
    let job = plan(&pages, &settings).expect("plan");
    let fonts = FontManager::new();
    let raster = render_planned(&pages.pages[0], &fonts, &job.pages[0], &job).expect("render");
    let inside = pixel(&raster, 38, 38);
    assert_eq!(inside[0], inside[1]);
    assert_eq!(inside[1], inside[2]);
    assert!(inside[0] > 0 && inside[0] < 255, "red becomes mid grey");
}

#[test]
fn the_whole_job_renders_in_print_order() {
    let settings = PrintSettings {
        paper: PaperSize::Document,
        dpi: 72.0,
        ..PrintSettings::default()
    };
    let pages = list(vec![page(0, 20.0, 20.0), page(1, 20.0, 20.0)]);
    let job = plan(&pages, &settings).expect("plan");
    let fonts = FontManager::new();
    let rasters = render_job(&pages, &fonts, &job).expect("render all");
    assert_eq!(rasters.len(), 2);
    assert_eq!(rasters[0].width, rasters[1].width);
}

#[test]
fn a_reversed_job_prints_the_last_page_first() {
    let settings = PrintSettings {
        paper: PaperSize::Document,
        dpi: 72.0,
        selection: sofd_print::PageSelection::List(vec![1, 2]),
        order: sofd_print::PageOrder::Reverse,
        ..PrintSettings::default()
    };
    let pages = list(vec![page(0, 20.0, 20.0), page(1, 20.0, 20.0)]);
    let job = plan(&pages, &settings).expect("plan");
    let order: Vec<usize> = job.pages.iter().map(|p| p.page_index).collect();
    assert_eq!(order, vec![1, 0]);
}

#[test]
fn a_simplex_job_uses_one_sheet_per_page() {
    let settings = PrintSettings {
        paper: PaperSize::Document,
        dpi: 72.0,
        duplex: Duplex::None,
        ..PrintSettings::default()
    };
    let pages = list(vec![page(0, 20.0, 20.0), page(1, 20.0, 20.0)]);
    let job = plan(&pages, &settings).expect("plan");
    assert_eq!(job.sheet_count(), 2);
    assert_eq!(job.pages[0].sheet, 0);
    assert_eq!(job.pages[1].sheet, 1);
}

#[test]
fn a_zero_dpi_is_rejected_before_any_rendering() {
    let settings = PrintSettings {
        dpi: 0.0,
        ..PrintSettings::default()
    };
    let err = plan(&list(vec![page(0, 20.0, 20.0)]), &settings).expect_err("no dpi");
    assert!(matches!(
        err,
        OfdError::Print(PrintError::InvalidScale { .. })
    ));
}

#[test]
fn a_negative_custom_scale_is_rejected() {
    let settings = PrintSettings {
        scale: ScaleMode::Custom(-50.0),
        ..PrintSettings::default()
    };
    let err = plan(&list(vec![page(0, 20.0, 20.0)]), &settings).expect_err("no scale");
    assert!(matches!(
        err,
        OfdError::Print(PrintError::InvalidScale { .. })
    ));
}

#[test]
fn orientation_swaps_only_when_needed() {
    assert_eq!(Orientation::Portrait.orient(210.0, 297.0), (210.0, 297.0));
    assert_eq!(Orientation::Portrait.orient(297.0, 210.0), (210.0, 297.0));
    assert_eq!(Orientation::Landscape.orient(210.0, 297.0), (297.0, 210.0));
    assert_eq!(Orientation::Landscape.orient(297.0, 210.0), (297.0, 210.0));
}
