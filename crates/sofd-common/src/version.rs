//! Version identifiers.

use serde::{Deserialize, Serialize};

/// OFD document version, from the `Version` attribute of `OFD.xml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfdVersion {
    /// National standard GB/T 33190-2016.
    #[default]
    V1_0,
    /// Extended version used by e-invoices and vouchers.
    V1_1,
    /// Unrecognized version, keeping the original string.
    Other(String),
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
    /// Electronic official document format, part 1: document structure.
    pub const GB_T_33476_1: &'static str = "GB/T 33476.1-2016";
    /// Electronic official document format, part 2: presentation.
    pub const GB_T_33476_2: &'static str = "GB/T 33476.2-2016";
    /// Cryptographic specification for secure electronic seals.
    pub const GB_T_38540: &'static str = "GB/T 38540-2020";
    /// Cryptography application for open fixed-layout documents.
    pub const GM_T_0099: &'static str = "GM/T 0099-2020";
    /// Archiving specification for electronic official documents.
    pub const GB_T_39362: &'static str = "GB/T 39362-2020";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_version_is_the_national_standard() {
        assert_eq!(OfdVersion::default(), OfdVersion::V1_0);
    }

    #[test]
    fn known_versions_round_trip_through_strings() {
        for v in [OfdVersion::V1_0, OfdVersion::V1_1] {
            assert_eq!(OfdVersion::parse(v.as_str()), v);
        }
    }

    #[test]
    fn unknown_versions_keep_their_raw_text() {
        let v = OfdVersion::parse("2.5");
        assert_eq!(v, OfdVersion::Other("2.5".to_string()));
        assert_eq!(v.as_str(), "2.5");
    }

    #[test]
    fn empty_and_malformed_versions_are_preserved_verbatim() {
        assert_eq!(OfdVersion::parse("").as_str(), "");
        assert_eq!(OfdVersion::parse("not-a-version").as_str(), "not-a-version");
    }

    #[test]
    fn spec_constants_use_the_official_standard_numbers() {
        assert_eq!(SpecVersion::GB_T_33190, "GB/T 33190-2016");
        assert_eq!(SpecVersion::GB_T_33476_1, "GB/T 33476.1-2016");
        assert_eq!(SpecVersion::GB_T_33476_2, "GB/T 33476.2-2016");
        assert_eq!(SpecVersion::GB_T_38540, "GB/T 38540-2020");
        assert_eq!(SpecVersion::GM_T_0099, "GM/T 0099-2020");
        assert_eq!(SpecVersion::GB_T_39362, "GB/T 39362-2020");
    }
}
