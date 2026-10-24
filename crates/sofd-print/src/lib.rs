//! sofd-print
//! Turns a display list into a print job: paper, placement and raster pages.
//! The same display list the screen renders from, so output matches preview.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod job;
pub mod paper;
pub mod render;
pub mod settings;

pub use job::{PageSide, PlannedPage, PrintJob, plan};
pub use paper::{ColorMode, Duplex, Margins, Orientation, PaperSize};
pub use render::{RasterPage, render_job, render_planned};
pub use settings::{PageOrder, PageSelection, PrintSettings, ScaleMode};
