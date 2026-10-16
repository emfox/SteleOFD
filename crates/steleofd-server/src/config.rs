//! Settings the process reads from its environment.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

/// Environment variable naming the address to listen on.
pub const BIND: &str = "STELEOFD_BIND";
/// Environment variable naming the largest accepted upload.
pub const MAX_BODY: &str = "STELEOFD_MAX_BODY";
/// Environment variable naming how many documents may stay open.
pub const MAX_DOCUMENTS: &str = "STELEOFD_MAX_DOCUMENTS";
/// Environment variable naming the directory documents are kept in. When it is
/// set, an id survives a restart; when it is unset the store is memory only.
pub const DATA_DIR: &str = "STELEOFD_DATA_DIR";

/// Default listen address: loopback only, so a trial run is not a service.
pub const DEFAULT_BIND: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080));

/// Where the service listens and how much it accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Address to bind.
    pub bind: SocketAddr,
    /// Largest uploaded document, in bytes.
    pub max_body_bytes: usize,
    /// How many documents may stay open at once.
    pub max_documents: usize,
    /// Directory documents are kept in, when the service is persistent.
    pub data_dir: Option<std::path::PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND,
            max_body_bytes: 64 << 20,
            max_documents: 256,
            data_dir: None,
        }
    }
}

impl Settings {
    /// Read the settings from the process environment.
    pub fn from_env() -> Result<Self, BadSetting> {
        Self::from(|name| std::env::var(name).ok())
    }

    /// Read the settings from a lookup, which tests drive directly.
    pub fn from(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, BadSetting> {
        let defaults = Self::default();
        let text = |name: &str| lookup(name);
        let bind = match text(BIND) {
            Some(value) => value.parse().map_err(|_| BadSetting::new(BIND, &value))?,
            None => defaults.bind,
        };
        Ok(Settings {
            bind,
            max_body_bytes: number(MAX_BODY, text, defaults.max_body_bytes)?,
            max_documents: number(MAX_DOCUMENTS, text, defaults.max_documents)?,
            data_dir: text(DATA_DIR).filter(|dir| !dir.is_empty()).map(Into::into),
        })
    }
}

/// One setting that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadSetting {
    name: &'static str,
    value: String,
}

impl BadSetting {
    /// A value that did not parse, naming the variable it came from.
    pub fn new(name: &'static str, value: &str) -> Self {
        Self {
            name,
            value: value.to_owned(),
        }
    }
}

impl std::fmt::Display for BadSetting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is not usable: {}", self.name, self.value)
    }
}

impl std::error::Error for BadSetting {}

/// Parse a positive number, falling back to the default when unset.
fn number(
    name: &'static str,
    text: impl Fn(&str) -> Option<String>,
    fallback: usize,
) -> Result<usize, BadSetting> {
    match text(name) {
        Some(value) => {
            let parsed: usize = value.parse().map_err(|_| BadSetting::new(name, &value))?;
            if parsed == 0 {
                return Err(BadSetting::new(name, &value));
            }
            Ok(parsed)
        }
        None => Ok(fallback),
    }
}
