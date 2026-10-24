//! Geometry and transforms.
//! Backend-independent [`Path`] and [`Affine`]. Curve math uses `kurbo` behind our
//! own API, so a breaking dependency upgrade does not ripple outward.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod affine;
pub mod arc;
pub mod path;

pub use affine::Affine;
pub use arc::arc_cubics;
pub use path::{Cubic, Path, PathBuilder, PathSegment};
