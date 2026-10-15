//! Render backends.
//! Every backend consumes a [`steleofd_display::PageDisplay`] and translates
//! the [`steleofd_display::DrawOp`] stream into its own drawing model.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod raster;

pub use raster::{Cubic, RasterOutput, RenderOptions, arc_cubics, render_page};

// TODO: self-built clip stack, native tiny-skia clipping is enough for now.
// TODO: font subsetting for PDF and SVG export, currently embedded in full.
// TODO: bitmap render cache and page-window eviction, deferred until measured.
