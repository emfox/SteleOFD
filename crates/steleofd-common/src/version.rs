//! Version identifiers.

use serde::{Deserialize, Serialize};

/// OFD document version, from the `Version` attribute of `OFD.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfdVersion {
    /// National standard GB/T 33190-2016.
    V1_0,
    /// Extended version used by e-invoices and vouchers.
    V1_1,
    /// Unrecognized version, keeping the original string.
    Other(String),
}

impl Default for OfdVersion {
    fn default() -> Self {
        Self::V1_0
    }
}

impl OfdVersion {
    /// Parse a raw version string.
    pub fn parse(s: &str) -> Self {
        match s {
            "1.0" => Self::V1_0,
            "1.1" => Self::V1_1,
            other => Self::Other(other.to_string()),
        }
    }

    /// Render back to the canonical string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::V1_0 => "1.0",
            Self::V1_1 => "1.1",
            Self::Other(s) => s,
        }
    }
}

/// Standards this product claims conformance to, used for compliance reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecVersion;

impl SpecVersion {
    /// Fixed-layout document format.
    pub const GB_T_33190: &'static str = "GB/T 33190-2016";
    /// Electronic official document format for Party and government bodies.
    pub const GB_T_33476: &'static str = "GB/T 33476-2016";
    /// Cryptographic specification for secure electronic seals.
    pub const GB_T_38540: &'static str = "GB/T 38540-2020";
    /// OFD signature, encryption and integrity protection protocol.
    pub const GM_T_0099: &'static str = "GM/T 0099-2020";
    /// Archiving specification for electronic official documents.
    pub const GB_T_39362: &'static str = "GB/T 39362-2020";
}
