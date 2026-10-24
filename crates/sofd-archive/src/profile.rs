//! Producing an OFD-A container. GB/T 42133-2022 asks an authoring tool for
//! things a read-only checker cannot decide: strip the optional structures
//! clause 6.2.2 names. This module does exactly that, and nothing else.

use sofd_error::Result;
use sofd_package::{build_archive, resolve};

use crate::audit::OFD_A_DOC_TYPE;

/// Optional structures clause 6.2.2 wants out of an archived document. Each is
/// an element the 2016 schema allows but the archival profile drops.
const OPTIONAL_ELEMENTS: &[&str] = &[
    "Permissions",
    "VPreferences",
    "Extensions",
    "Keywords",
    "Attachments",
];

/// The container entries clause 6.2.2 wants gone: attachments and their list.
const OPTIONAL_ENTRIES: &[&str] = &["Attachments", "Attachment"];

/// What one call to [`archive`] changed, so a caller can log or assert on it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Archived {
    /// Container entries removed, in path order.
    pub removed_entries: Vec<String>,
    /// XML documents whose text had optional elements stripped.
    pub stripped_documents: Vec<String>,
    /// How many optional elements were removed across those documents.
    pub stripped_elements: usize,
    /// Whether the container's `DocType` had to be rewritten to `OFD-A`.
    pub declared_profile: bool,
}

impl Archived {
    /// Whether the container came back byte-identical.
    pub fn is_noop(&self) -> bool {
        self.removed_entries.is_empty() && self.stripped_elements == 0 && !self.declared_profile
    }
}

/// Rewrite `entries` into an OFD-A container and return its bytes. The caller
/// supplies the source container as `(path, bytes)` pairs; the result is a
/// fresh archive an archiver can write out.
pub fn archive(entries: &[(&str, &[u8])]) -> Result<(Vec<u8>, Archived)> {
    let mut change = Archived::default();
    let mut kept: Vec<(String, Vec<u8>)> = Vec::new();
    for (path, data) in entries {
        if is_optional_entry(path) {
            change.removed_entries.push((*path).to_string());
            continue;
        }
        if is_xml(path) {
            let (rewritten, removed) = strip_optional(data)?;
            if removed > 0 {
                change.stripped_documents.push((*path).to_string());
                change.stripped_elements += removed;
                kept.push(((*path).to_string(), rewritten));
                continue;
            }
        }
        kept.push(((*path).to_string(), data.to_vec()));
    }
    // Clause 6.2.1 a): the profile has to be declared, or the result is a
    // plain OFD by the standard's own test.
    if let Some(entry) = kept.iter_mut().find(|(path, _)| path == "OFD.xml") {
        let rewritten = declare_profile(&entry.1);
        change.declared_profile = rewritten != entry.1;
        entry.1 = rewritten;
    }
    let borrowed: Vec<(&str, &[u8])> = kept
        .iter()
        .map(|(path, data)| (path.as_str(), data.as_slice()))
        .collect();
    Ok((build_archive(&borrowed)?, change))
}

/// Whether a container entry is one clause 6.2.2 strips.
fn is_optional_entry(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.split('.').next().unwrap_or(name);
    OPTIONAL_ENTRIES.contains(&stem) || path.contains("/Attachments/")
}

/// Whether a path names an XML document to rewrite.
fn is_xml(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".xml")
}

/// Remove the optional elements from one XML document, keeping everything else
/// byte-for-byte. Returns the rewritten document and the count of removals.
fn strip_optional(data: &[u8]) -> Result<(Vec<u8>, usize)> {
    let Ok(text) = std::str::from_utf8(data) else {
        return Ok((data.to_vec(), 0));
    };
    let mut out = String::with_capacity(text.len());
    let mut removed = 0;
    let mut skip: Option<usize> = None;
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        rest = &rest[open..];
        let Some(close) = rest.find('>') else {
            out.push_str(rest);
            break;
        };
        let tag = &rest[..=close];
        rest = &rest[close + 1..];
        if let Some(depth) = skip.as_mut() {
            if tag.starts_with("</") {
                *depth -= 1;
                if *depth == 0 {
                    skip = None;
                }
            } else if !tag.starts_with("<?") && !tag.starts_with("<!") && !tag.ends_with("/>") {
                *depth += 1;
            }
            continue;
        }
        let Some(name) = element_name(tag) else {
            out.push_str(tag);
            continue;
        };
        if OPTIONAL_ELEMENTS.contains(&name.as_str()) {
            removed += 1;
            if !tag.ends_with("/>") && !tag.starts_with("</") {
                skip = Some(1);
            }
            continue;
        }
        out.push_str(tag);
    }
    Ok((out.into_bytes(), removed))
}

/// The element name a tag writes, or `None` for a declaration or comment.
fn element_name(tag: &str) -> Option<String> {
    if tag.starts_with("<?") || tag.starts_with("<!") {
        return None;
    }
    let body = tag.trim_start_matches('<').trim_start_matches('/');
    let end = body
        .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .unwrap_or(body.len());
    let name = &body[..end];
    let local = name.rsplit(':').next().unwrap_or(name);
    Some(local.to_string())
}

/// The byte offset of the `OFD` root tag, whatever namespace prefix it wears.
fn find_root_tag(text: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(rel) = text[from..].find("<OFD") {
        let at = from + rel;
        let after = text[at + 4..].chars().next();
        if matches!(after, Some(' ') | Some('>') | Some('/') | None) {
            return Some(at);
        }
        // `<ofd:OFD` also lands here; check the prefixed spelling too.
        from = at + 1;
    }
    let mut from = 0;
    while let Some(rel) = text[from..].find(":OFD") {
        let at = from + rel;
        let after = text[at + 4..].chars().next();
        if matches!(after, Some(' ') | Some('>') | Some('/') | None) {
            let start = text[..at].rfind('<')?;
            return Some(start);
        }
        from = at + 1;
    }
    None
}

/// Rewrite the `DocType` of `OFD.xml` to the archival profile, which is the
/// attribute the container already uses to name its type.
fn declare_profile(data: &[u8]) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(data) else {
        return data.to_vec();
    };
    let Some(at) = find_root_tag(text) else {
        return data.to_vec();
    };
    let Some(gt) = text[at..].find('>') else {
        return data.to_vec();
    };
    let tag = &text[at..=at + gt];
    let cut = at + gt + 1;
    let rewritten = match tag.find("DocType=\"") {
        Some(at) => {
            let start = at + "DocType=\"".len();
            let end = tag[start..].find('"').map(|e| start + e).unwrap_or(start);
            let mut tag = String::with_capacity(tag.len());
            tag.push_str(&text[..at]);
            tag.push_str("DocType=\"");
            tag.push_str(OFD_A_DOC_TYPE);
            tag.push_str(&text[end..cut]);
            tag
        }
        None => {
            let mut tag = String::with_capacity(tag.len() + 24);
            tag.push_str(&text[..cut - 1]);
            tag.push_str(&format!(" DocType=\"{OFD_A_DOC_TYPE}\""));
            tag.push('>');
            tag
        }
    };
    let mut out = String::with_capacity(text.len() + 24);
    out.push_str(&rewritten);
    out.push_str(&text[cut..]);
    out.into_bytes()
}

/// Resolve an attachment reference to its container path, so a caller can name
/// the entry clause 6.2.2 removes for a given document.
pub fn attachment_path(base: &str, reference: &str) -> String {
    resolve(base, reference)
}
