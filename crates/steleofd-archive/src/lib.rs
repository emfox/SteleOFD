//! steleofd-archive
//! Archival readiness auditing derived from GB/T 33190-2016.
//! This is not OFD/A certification; the checks cover self-containment only.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod report;

pub use audit::{Auditor, audit};
pub use report::{Finding, Report, Severity};
