//! steleofd-core: the feature facade over the OFD stack. One handle opens a
//! document; rendering, conversion, printing, signing, editing, search and
//! auditing each sit behind their own cargo feature.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod document;

pub use document::Document;
