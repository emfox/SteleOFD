//! OFD document model.
//! Mirrors GB/T 33190-2016 as pure data with no I/O, so parsing, writing and
//! tests share these types. Element names follow the standard exactly.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod color;
pub mod document;
pub mod object;

pub use color::{ColorRef, Segment, Shading};
pub use document::{
    CommonData, DocBody, DocRoot, Document, Location, OfdRoot, Page, PageBlock, Pages, Version,
    Zones,
};
pub use object::{
    ClipArea, ClipList, DashPattern, DrawParam, FillRuleStyle, GraphicsUnit, ImageObject,
    LineCapStyle, LineJoinStyle, Object, ObjectRef, PathObject, TextObject,
};

/// Reference attribute (`ID` in the standard) identifying an object or resource.
pub type RefId = String;

/// Whether all required fields are present in a parsed structure.
/// OFD makes many elements optional, so validation is a separate step from parsing.
pub trait Valid {
    /// Returns `Ok(())` when the structure is internally consistent.
    fn validate(&self) -> steleofd_error::Result<()>;
}
