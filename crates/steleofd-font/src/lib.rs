//! Font management, metrics and shaping.
//! Handles embedded fonts, system-resolved names and bare CID indices, plus the
//! reverse glyph-to-Unicode lookup that search and copy depend on.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod cff;
pub mod face;
pub mod manager;
pub mod metrics;
pub mod registry;
pub mod shape;
pub mod subset;

pub use face::FontFace;
pub use manager::FontManager;
pub use metrics::{FontMetrics, GlyphOutline};
pub use registry::{FontId, FontRegistry, FontSource, FontTraits, Unresolved};
pub use shape::{ShapeOptions, ShapedGlyph, ShapedRun, shape_text};
pub use subset::subset;
