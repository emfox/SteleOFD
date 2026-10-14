//! Colour representation as written in OFD.
//! The standard writes colours as `#RRGGBB` or `rgb(r,g,b)`, sometimes with an
//! accompanying alpha; this keeps the text form so a round trip is lossless.

use serde::{Deserialize, Serialize};
use steleofd_error::{OfdError, XmlError};

use crate::Valid;

/// Colour vocabulary of GB/T 33190-2016, used by `FillColor` and `StrokeColor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorRef {
    /// `#RRGGBB` or `rgb(r,g,b)` as written.
    pub value: String,
    /// Alpha from 0 to 255, absent meaning opaque.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha: Option<u8>,
}

impl ColorRef {
    /// Opaque black, the OFD default when no colour is written.
    pub const BLACK: Self = Self {
        value: String::new(),
        alpha: None,
    };

    /// Build from an `#RRGGBB` string.
    pub fn from_hex(hex: impl Into<String>) -> Self {
        Self {
            value: hex.into(),
            alpha: None,
        }
    }

    /// Attach an alpha value.
    pub fn with_alpha(mut self, alpha: u8) -> Self {
        self.alpha = Some(alpha);
        self
    }

    /// Whether the colour text is absent, meaning the OFD default applies.
    pub fn is_default(&self) -> bool {
        self.value.is_empty()
    }

    /// Parse `#RGB`, `#RRGGBB` or `rgb(r,g,b)` into channels.
    pub fn to_rgb(&self) -> steleofd_error::Result<(u8, u8, u8)> {
        if self.is_default() {
            return Ok((0, 0, 0));
        }

        let s = self.value.trim();
        if s.starts_with('#') {
            return parse_hex(s.trim_start_matches('#'));
        }
        if let Some(inner) = s.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
            return parse_triple(inner);
        }
        // Written without a prefix at all.
        parse_hex(s)
    }

    /// Alpha, defaulting to opaque.
    pub fn alpha(self) -> u8 {
        self.alpha.unwrap_or(255)
    }
}

impl Valid for ColorRef {
    fn validate(&self) -> steleofd_error::Result<()> {
        self.to_rgb().map(|_| ())
    }
}

/// Parse a 3- or 6-digit hex string into channels.
fn parse_hex(s: &str) -> steleofd_error::Result<(u8, u8, u8)> {
    match s.len() {
        6 => {
            let v = u32::from_str_radix(s, 16).map_err(|_| invalid(s))?;
            Ok((
                ((v >> 16) & 0xFF) as u8,
                ((v >> 8) & 0xFF) as u8,
                (v & 0xFF) as u8,
            ))
        }
        // The three-digit form expands each digit, so `abc` is `aabbcc`.
        3 => {
            let v = u32::from_str_radix(s, 16).map_err(|_| invalid(s))?;
            let expand = |n: u32| ((n & 0xF) * 0x11) as u8;
            Ok((expand(v >> 8), expand(v >> 4), expand(v)))
        }
        _ => Err(invalid(s)),
    }
}

/// Parse a comma-separated triple into channels.
fn parse_triple(inner: &str) -> steleofd_error::Result<(u8, u8, u8)> {
    let nums: Result<Vec<u8>, _> = inner.split(',').map(|p| p.trim().parse::<u8>()).collect();
    match nums {
        Ok(v) if v.len() == 3 => Ok((v[0], v[1], v[2])),
        _ => Err(invalid(inner)),
    }
}

fn invalid(value: &str) -> OfdError {
    OfdError::Xml(XmlError::InvalidAttribute {
        element: "Color".to_string(),
        attr: "Value".to_string(),
        value: value.to_string(),
    })
}

impl std::fmt::Display for ColorRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.alpha {
            Some(a) => write!(f, "{} alpha={}", self.value, a),
            None => f.write_str(&self.value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_digit_hex_parses_to_channels() {
        let c = ColorRef::from_hex("#FF8000");
        assert_eq!(c.to_rgb().expect("parse"), (255, 128, 0));
    }

    #[test]
    fn six_digit_hex_without_the_hash_parses_the_same() {
        let c = ColorRef::from_hex("FF8000");
        assert_eq!(c.to_rgb().expect("parse"), (255, 128, 0));
    }

    #[test]
    fn three_digit_hex_expands_each_channel() {
        // #F80 must mean the same colour as #FF8800.
        let short = ColorRef::from_hex("#F80");
        let long = ColorRef::from_hex("#FF8800");
        assert_eq!(
            short.to_rgb().expect("parse"),
            long.to_rgb().expect("parse")
        );
    }

    #[test]
    fn whitespaces_in_hex_are_tolerated() {
        let c = ColorRef::from_hex("  #FF8000  ");
        assert_eq!(c.to_rgb().expect("parse"), (255, 128, 0));
    }

    #[test]
    fn rgb_function_syntax_parses() {
        let c = ColorRef::from_hex("rgb(10, 20, 30)");
        assert_eq!(c.to_rgb().expect("parse"), (10, 20, 30));
    }

    #[test]
    fn garbage_reports_the_value() {
        let c = ColorRef::from_hex("notacolour");
        let err = c.to_rgb().expect_err("must fail");
        match err {
            OfdError::Xml(XmlError::InvalidAttribute { element, value, .. }) => {
                assert_eq!(element, "Color");
                assert_eq!(value, "notacolour");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn a_missing_colour_falls_back_to_black() {
        let c = ColorRef::BLACK;
        assert!(c.is_default());
        assert_eq!(c.to_rgb().expect("parse"), (0, 0, 0));
    }

    #[test]
    fn alpha_defaults_to_opaque() {
        let c = ColorRef::from_hex("#FFFFFF");
        assert_eq!(c.clone().alpha(), 255);
        assert_eq!(c.with_alpha(128).alpha(), 128);
    }

    #[test]
    fn display_shows_alpha_only_when_present() {
        assert_eq!(ColorRef::from_hex("#FFF").to_string(), "#FFF");
        assert_eq!(
            ColorRef::from_hex("#FFF").with_alpha(64).to_string(),
            "#FFF alpha=64"
        );
    }

    #[test]
    fn validation_accepts_well_formed_colours() {
        assert!(ColorRef::from_hex("#AABBCC").validate().is_ok());
        assert!(ColorRef::from_hex("zzz").validate().is_err());
    }

    #[test]
    fn colours_degrade_gracefully_when_serde_round_trips() {
        let c = ColorRef::from_hex("#123456").with_alpha(10);
        let json = serde_json::to_string(&c).expect("serialize");
        let back: ColorRef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, c);
    }
}
