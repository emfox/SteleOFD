//! Display list intermediate representation.
//! Flattens the nested OFD template tree once into a [`DisplayList`] of ops with
//! absolute transforms, so render, print, convert and search share one IR.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod jbig2;
pub mod list;
pub mod ops;
pub mod page;
pub mod resource;
pub mod shading;
pub mod text;

pub use list::{DisplayList, DisplayListBuilder, PageDisplay, PageRef};
pub use ops::{DrawOp, Paint, PaintRule, StrokeStyle};
pub use page::{PageInfo, RenderQuality, Viewport};
pub use resource::{ImageCodec, ImageRef, ResolvedFont, ResolvedResources};
pub use shading::{FocalCone, RadialForm, radial_form};
pub use text::{GlyphRun, TextSpan};
