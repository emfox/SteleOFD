//! Render backends.
//! Every backend consumes a [`steleofd_display::PageDisplay`] and translates the
//! eight [`steleofd_display::DrawOp`] variants into its own drawing model.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

// TODO: self-built clip stack, native tiny-skia clipping is enough for now.
// TODO: font subsetting for PDF and SVG export, currently embedded in full.
// TODO: bitmap render cache and page-window eviction, deferred until measured.
