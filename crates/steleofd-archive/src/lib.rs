//! steleofd-archive
//! Archival readiness auditing from GB/T 33190-2016 and GB/T 42133-2022. It
//! reports what those standards require; it certifies nothing and rewrites nothing.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod profile;
pub mod report;

pub use audit::{Auditor, audit};
pub use profile::{Archived, archive};
pub use report::{Finding, Report, Severity};
