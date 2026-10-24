//! Panic isolation at the boundary with a font backend, which asserts on bytes
//! a document supplies. A guard only helps where unwinding exists: under the
//! release profile's `panic = "abort"` a panic is a process exit, not a value.

use std::panic::{self, UnwindSafe};

use sofd_error::{FontError, Result};

/// Run `f` on font bytes, turning a panic into a parse error. The message is
/// still printed, so a panicking backend is visible in a log; what the guard
/// adds is that the caller gets a value back for one malformed font.
pub(crate) fn guard<T>(what: &str, f: impl FnOnce() -> Result<T> + UnwindSafe) -> Result<T> {
    match panic::catch_unwind(f) {
        Ok(result) => result,
        Err(_) => {
            Err(FontError::Parse(format!("the backend panicked while reading {what}")).into())
        }
    }
}
