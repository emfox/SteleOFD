//! The HTTP delivery path: a document service over the `core` facade. Uploading
//! an `.ofd` file yields a document id; every other route works on that id, so
//! a caller renders page after page without resending the file.

#![warn(missing_docs)]

pub mod api;
pub mod config;
pub mod dto;
pub mod error;
pub mod store;

pub use config::{BadSetting, Settings};
pub use error::ApiError;
pub use store::Store;
