//! Settings the process reads from its environment.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

/// Environment variable naming the address to listen on.
pub const BIND: &str = "SOFD_BIND";
/// Environment variable naming the largest accepted upload.
pub const MAX_BODY: &str = "SOFD_MAX_BODY";
/// Environment variable naming how many documents may stay open.
pub const MAX_DOCUMENTS: &str = "SOFD_MAX_DOCUMENTS";
/// Environment variable naming the directory documents are kept in. When it is
/// set, an id survives a restart; when it is unset the store is memory only.
pub const DATA_DIR: &str = "SOFD_DATA_DIR";
/// Environment variable naming the API keys, as `tenant:key,tenant:key`. When
/// it is unset the service accepts every caller as one shared tenant.
pub const API_KEYS: &str = "SOFD_API_KEYS";
/// Environment variable setting the per-caller request rate and burst, as
/// `rate,burst`. Unset means a limit generous enough never to be met.
pub const RATE_LIMIT: &str = "SOFD_RATE_LIMIT";
/// Environment variable setting how many seconds in-flight work is given to
/// finish after a stop is asked for. Unset means the generous default.
pub const DRAIN_TIMEOUT: &str = "SOFD_DRAIN_TIMEOUT";

/// Default listen address: loopback only, so a trial run is not a service.
pub const DEFAULT_BIND: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080));

/// Where the service listens and how much it accepts.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Address to bind.
    pub bind: SocketAddr,
    /// Largest uploaded document, in bytes.
    pub max_body_bytes: usize,
    /// How many documents may stay open at once.
    pub max_documents: usize,
    /// Directory documents are kept in, when the service is persistent.
    pub data_dir: Option<std::path::PathBuf>,
    /// The API keys the service accepts. Empty means every caller is one
    /// shared tenant.
    pub api_keys: crate::tenancy::KeyRing,
    /// How often each caller may ask for work.
    pub rate_limit: crate::ratelimit::RateLimit,
    /// How long in-flight work may take to finish after a stop.
    pub drain: crate::drain::Deadline,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND,
            max_body_bytes: 64 << 20,
            max_documents: 256,
            data_dir: None,
            api_keys: crate::tenancy::KeyRing::new(),
            rate_limit: crate::ratelimit::RateLimit::generous(),
            drain: crate::drain::Deadline::generous(),
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
            api_keys: match text(API_KEYS) {
                Some(spec) => crate::tenancy::KeyRing::parse(&spec)
                    .map_err(|_| BadSetting::new(API_KEYS, &spec))?,
                None => defaults.api_keys,
            },
            rate_limit: match text(RATE_LIMIT) {
                Some(spec) => parse_rate(&spec)?,
                None => defaults.rate_limit,
            },
            drain: match text(DRAIN_TIMEOUT) {
                Some(spec) => parse_drain(&spec)?,
                None => defaults.drain,
            },
        })
    }
}

/// Parse `rate,burst` into a rate limit, refusing one that would refuse every
/// request.
fn parse_rate(spec: &str) -> Result<crate::ratelimit::RateLimit, BadSetting> {
    let bad = || BadSetting::new(RATE_LIMIT, spec);
    let (rate, burst) = spec.split_once(',').ok_or_else(bad)?;
    let rate: f64 = rate.trim().parse().map_err(|_| bad())?;
    let burst: f64 = burst.trim().parse().map_err(|_| bad())?;
    let limit = crate::ratelimit::RateLimit::new(rate, burst);
    if !limit.is_usable() {
        return Err(bad());
    }
    Ok(limit)
}

/// Parse a grace period in seconds, refusing zero: a drain of no time is a
/// kill, which is the opposite of what the setting is for.
fn parse_drain(spec: &str) -> Result<crate::drain::Deadline, BadSetting> {
    let bad = || BadSetting::new(DRAIN_TIMEOUT, spec);
    let seconds: f64 = spec.trim().parse().map_err(|_| bad())?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(bad());
    }
    Ok(crate::drain::Deadline::new(
        std::time::Duration::from_secs_f64(seconds),
    ))
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
