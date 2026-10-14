//! XML reading for OFD documents.
//! OFD attributes are abbreviated, namespace prefixes vary between writers, and
//! values carry units; this layer types them and maps failures onto [`XmlError`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod attribute;
pub mod cursor;

pub use attribute::{AttrValue, Attributes};
pub use cursor::{Cursor, Node};

use steleofd_error::{OfdError, XmlError};

/// The namespace OFD documents declare.
pub const OFD_NS: &str = "http://www.ofdspec.org/2016";

/// Strip a namespace prefix from a qualified name.
/// Writers disagree on prefixes (`ofd:Page` versus `Page`), so matching is always
/// done on the local name.
pub fn local_name(qualified: &str) -> &str {
    match qualified.rsplit_once(':') {
        Some((_, local)) => local,
        None => qualified,
    }
}

/// Namespace prefix of a qualified name, empty when unprefixed.
pub fn prefix_of(qualified: &str) -> &str {
    match qualified.split_once(':') {
        Some((prefix, _)) => prefix,
        None => "",
    }
}

/// Convert a `quick-xml` failure into an XML error with a best-effort position.
pub(crate) fn map_error(err: &quick_xml::Error, pos: (u64, u64)) -> OfdError {
    let (line, col) = pos;
    let msg = match err {
        quick_xml::Error::Io(e) => format!("I/O failed: {e}"),
        other => other.to_string(),
    };
    XmlError::Syntax {
        line,
        col,
        msg: clean_message(&msg),
    }
    .into()
}

/// Strip the reader's byte-offset noise so messages read as prose.
fn clean_message(msg: &str) -> String {
    let first_line = msg.lines().next().unwrap_or(msg).trim();
    first_line.trim_end_matches('.').to_string()
}

/// Parse a required attribute value into `T`.
pub fn parse_required<T>(element: &str, attr: &str, raw: Option<&str>) -> steleofd_error::Result<T>
where
    T: std::str::FromStr,
{
    let raw = raw.ok_or_else(|| {
        OfdError::Xml(XmlError::MissingAttribute {
            element: element.to_string(),
            attr: attr.to_string(),
        })
    })?;
    raw.trim().parse::<T>().map_err(|_| {
        OfdError::Xml(XmlError::InvalidAttribute {
            element: element.to_string(),
            attr: attr.to_string(),
            value: raw.to_string(),
        })
    })
}

/// Parse an optional attribute value into `T`.
/// An absent attribute yields `None`; a present-but-malformed one is an error
/// rather than a silent default, since that usually means a corrupt document.
pub fn parse_optional<T>(
    element: &str,
    attr: &str,
    raw: Option<&str>,
) -> steleofd_error::Result<Option<T>>
where
    T: std::str::FromStr,
{
    match raw {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(None),
        Some(v) => v.trim().parse::<T>().map(Some).map_err(|_| {
            OfdError::Xml(XmlError::InvalidAttribute {
                element: element.to_string(),
                attr: attr.to_string(),
                value: v.to_string(),
            })
        }),
    }
}

/// Split a whitespace-separated number list, the OFD `Boundary` and `CTM` form.
pub fn parse_number_list<T>(element: &str, attr: &str, raw: &str) -> steleofd_error::Result<Vec<T>>
where
    T: std::str::FromStr,
{
    let mut out = Vec::new();
    for tok in raw.split_whitespace() {
        out.push(tok.parse::<T>().map_err(|_| {
            OfdError::Xml(XmlError::InvalidAttribute {
                element: element.to_string(),
                attr: attr.to_string(),
                value: raw.to_string(),
            })
        })?);
    }
    Ok(out)
}

/// Parse a fixed-arity number list, failing when the count does not match.
pub fn parse_number_list_n<const N: usize>(
    element: &str,
    attr: &str,
    raw: &str,
) -> steleofd_error::Result<[f64; N]> {
    let values = parse_number_list::<f64>(element, attr, raw)?;
    values.try_into().map_err(|v: Vec<f64>| {
        OfdError::Xml(XmlError::InvalidAttribute {
            element: element.to_string(),
            attr: attr.to_string(),
            value: format!("{raw} (expected {N} numbers, got {})", v.len()),
        })
    })
}

/// Parse an OFD boolean, written `true`/`false` but sometimes `1`/`0`.
pub fn parse_bool(element: &str, attr: &str, raw: &str) -> steleofd_error::Result<bool> {
    match raw.trim() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        other => Err(OfdError::Xml(XmlError::InvalidAttribute {
            element: element.to_string(),
            attr: attr.to_string(),
            value: other.to_string(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_name_strips_any_prefix() {
        assert_eq!(local_name("ofd:Page"), "Page");
        assert_eq!(local_name("Page"), "Page");
        // A name containing a colon only in the local part keeps the last segment.
        assert_eq!(local_name("a:b:Page"), "Page");
    }

    #[test]
    fn prefix_of_reports_the_namespace_prefix() {
        assert_eq!(prefix_of("ofd:Page"), "ofd");
        assert_eq!(prefix_of("Page"), "");
    }

    #[test]
    fn required_attributes_are_parsed_or_reported_missing() {
        let ok: u32 = parse_required("Page", "ID", Some("7")).expect("parse");
        assert_eq!(ok, 7);

        let err = parse_required::<u32>("Page", "ID", None).expect_err("must fail");
        match err {
            OfdError::Xml(XmlError::MissingAttribute { element, attr }) => {
                assert_eq!(element, "Page");
                assert_eq!(attr, "ID");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn malformed_required_attributes_name_the_value() {
        let err = parse_required::<f64>("Page", "BaseLoc", Some("abc")).expect_err("must fail");
        match err {
            OfdError::Xml(XmlError::InvalidAttribute {
                element,
                attr,
                value,
            }) => {
                assert_eq!(element, "Page");
                assert_eq!(attr, "BaseLoc");
                assert_eq!(value, "abc");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn optional_attributes_accept_absence_but_not_garbage() {
        assert_eq!(parse_optional::<u32>("T", "A", None).expect("parse"), None);
        assert_eq!(
            parse_optional::<u32>("T", "A", Some("  ")).expect("parse"),
            None
        );
        assert_eq!(
            parse_optional::<u32>("T", "A", Some("5")).expect("parse"),
            Some(5)
        );
        assert!(parse_optional::<u32>("T", "A", Some("x")).is_err());
    }

    #[test]
    fn boundary_lists_are_split_on_whitespace() {
        let v = parse_number_list::<f64>("Page", "Boundary", "0 0 210 297").expect("parse");
        assert_eq!(v, vec![0.0, 0.0, 210.0, 297.0]);
    }

    #[test]
    fn boundary_lists_tolerate_irregular_spacing() {
        let v = parse_number_list::<f64>("Page", "Boundary", "  0   0\t210\n297  ").expect("parse");
        assert_eq!(v, vec![0.0, 0.0, 210.0, 297.0]);
    }

    #[test]
    fn fixed_arity_lists_check_the_count() {
        let v = parse_number_list_n::<4>("Page", "Boundary", "0 0 210 297").expect("parse");
        assert_eq!(v, [0.0, 0.0, 210.0, 297.0]);

        let err = parse_number_list_n::<4>("Page", "Boundary", "0 0 210").expect_err("must fail");
        match err {
            OfdError::Xml(XmlError::InvalidAttribute { value, .. }) => {
                assert!(value.contains("expected 4"), "value was {value}");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn booleans_accept_both_spellings() {
        assert!(parse_bool("T", "A", "true").expect("parse"));
        assert!(parse_bool("T", "A", "1").expect("parse"));
        assert!(!parse_bool("T", "A", "false").expect("parse"));
        assert!(!parse_bool("T", "A", "0").expect("parse"));
        assert!(parse_bool("T", "A", "yes").is_err());
    }

    #[test]
    fn malformed_lists_report_the_offending_element() {
        let err = parse_number_list::<f64>("Page", "Boundary", "0 0 x 297").expect_err("must fail");
        match err {
            OfdError::Xml(XmlError::InvalidAttribute { element, attr, .. }) => {
                assert_eq!(element, "Page");
                assert_eq!(attr, "Boundary");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn the_ofd_namespace_constant_matches_the_standard() {
        assert_eq!(OFD_NS, "http://www.ofdspec.org/2016");
    }
}
