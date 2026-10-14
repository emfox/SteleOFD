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
