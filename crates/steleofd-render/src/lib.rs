//! Render backends.
//! Every backend consumes a [`steleofd_display::PageDisplay`] and translates
//! the [`steleofd_display::DrawOp`] stream into its own drawing model.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod raster;

pub use raster::{Cubic, RasterOutput, RenderOptions, arc_cubics, render_page};

// Subsetting is not this crate's to do: `steleofd-core` cuts each embedded face
// to the glyphs the display list names, and neither exporter carries a font;
// they outline every glyph into a path.

// The clip and the bitmap cache are deferred rather than settled, each waiting
// on a measurement rather than on a reason; `docs/Roadmap.md` carries both.
