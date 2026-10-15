//! Native desktop reader for OFD documents. The engine is linked directly, so
//! a document is opened, rendered and searched on the machine that holds it;
//! nothing is uploaded anywhere.

#![warn(missing_docs)]

pub mod app;
pub mod render_worker;
pub mod viewer;
pub mod window;

pub use app::{App, AppError, Canvas, Fit};
pub use render_worker::{RenderQueue, RenderRequest, RenderResult};
pub use viewer::{DroppedFile, PageCache, RenderedPage, looks_blank};
