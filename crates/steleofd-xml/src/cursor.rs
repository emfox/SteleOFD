//! Streaming element cursor.
//! [`Cursor`] turns the flat event stream into an element-oriented view, so
//! callers write document logic instead of event matching.

use steleofd_error::{OfdError, Result};

use crate::attribute::Attributes;
use crate::{local_name, map_error};

/// One node in the document, with attributes already decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// Element local name, prefix removed.
    local: String,
    /// Qualified name as written, kept for diagnostics.
    qualified: String,
    /// Decoded attributes.
    attributes: Attributes,
    /// Whether the tag closed itself (`<Foo/>`), so no `End` will follow.
    self_closing: bool,
}

impl Node {
    /// Local name with any namespace prefix removed.
    pub fn local(&self) -> &str {
        &self.local
    }

    /// Qualified name as it appeared in the source.
    pub fn qualified(&self) -> &str {
        &self.qualified
    }

    /// Attributes of this element.
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    /// Whether the local name matches.
    pub fn is(&self, name: &str) -> bool {
        self.local == name
    }

    /// Whether the local name is any of the candidates.
    pub fn is_any(&self, names: &[&str]) -> bool {
        names.contains(&self.local.as_str())
    }

    /// Whether this element closed itself and has no matching end tag.
    pub fn is_self_closing(&self) -> bool {
        self.self_closing
    }
}

/// A parsed event.
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    /// An opening tag, or a self-closing tag.
    Start(Node),
    /// A closing tag for the named element.
    End(String),
    /// Character data, already unescaped and trimmed.
    Text(String),
    /// The document ended.
    Eof,
}

/// Default nesting ceiling; OFD templates never nest this deeply, so exceeding
/// it means a malformed document or an attempt to exhaust the stack.
pub const DEFAULT_MAX_DEPTH: usize = 64;

/// Byte offset to a one-based line and column.
/// The reader reports offsets, not positions, so this is an approximation that
/// keeps errors useful without retaining the whole document.
fn position_of(offset: u64) -> (u64, u64) {
    (offset.saturating_div(40) + 1, 1)
}

/// Build a node from a start tag, decoding its attributes.
fn build_node(start: &quick_xml::events::BytesStart<'_>, self_closing: bool) -> Result<Node> {
    let qualified = start.name().as_ref().to_string();
    let local = local_name(&qualified).to_string();
    let attributes = read_attributes(start)?;
    Ok(Node {
        local,
        qualified,
        attributes,
        self_closing,
    })
}

/// Decode every attribute of a start tag.
fn read_attributes(start: &quick_xml::events::BytesStart<'_>) -> Result<Attributes> {
    let mut out = Attributes::default();
    for attr in start.attributes() {
        let attr = attr.map_err(|e| {
            OfdError::Xml(steleofd_error::XmlError::Syntax {
                line: 0,
                col: 0,
                msg: format!("malformed attribute: {e}"),
            })
        })?;
        let key = local_name(attr.key.as_ref()).to_string();
        // `value` is the raw text between the quotes; entities must be resolved
        // explicitly or `&amp;` would reach callers still escaped.
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|e| {
                OfdError::Xml(steleofd_error::XmlError::Syntax {
                    line: 0,
                    col: 0,
                    msg: format!("undecodable value for attribute {key}: {e}"),
                })
            })?
            .to_string();
        out.insert(key, value);
    }
    Ok(out)
}

/// A pull cursor over an XML document.
pub struct Cursor<'a> {
    reader: quick_xml::Reader<&'a [u8]>,
    buf: Vec<u8>,
    depth: usize,
    max_depth: usize,
    /// Elements opened but not yet closed, so a truncated document is caught.
    open: Vec<String>,
}

impl std::fmt::Debug for Cursor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cursor")
            .field("depth", &self.depth)
            .field("max_depth", &self.max_depth)
            .finish()
    }
}

impl<'a> Cursor<'a> {
    /// Start reading a document.
    pub fn new(xml: &'a str) -> Self {
        Self::with_max_depth(xml, DEFAULT_MAX_DEPTH)
    }

    /// Start reading with an explicit nesting ceiling.
    pub fn with_max_depth(xml: &'a str, max_depth: usize) -> Self {
        let mut reader = quick_xml::Reader::from_str(xml);
        let config = reader.config_mut();
        // OFD writes escaped entities in attributes and text, so unescaping stays
        // on; prefix stripping is handled here instead so local names are stable.
        config.trim_text(true);
        config.expand_empty_elements = false;
        Self {
            reader,
            buf: Vec::new(),
            depth: 0,
            max_depth,
            open: Vec::new(),
        }
    }

    /// Current nesting depth.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Read the next event.
    pub fn next_event(&mut self) -> Result<Event> {
        loop {
            let pos = self.reader.buffer_position();
            let event = match self.reader.read_event_into(&mut self.buf) {
                Ok(e) => e,
                Err(e) => {
                    let at = position_of(pos);
                    return Err(map_error(&e, at));
                }
            };

            match event {
                quick_xml::events::Event::Start(start) => {
                    self.depth += 1;
                    if self.depth > self.max_depth {
                        return Err(OfdError::Format(
                            steleofd_error::FormatError::MaxDepthExceeded(self.max_depth),
                        ));
                    }
                    let node = build_node(&start, false)?;
                    self.open.push(node.local().to_string());
                    return Ok(Event::Start(node));
                }
                quick_xml::events::Event::Empty(empty) => {
                    // A self-closing tag is a whole element on its own, so it is
                    // flagged rather than left waiting for an end tag.
                    let node = build_node(&empty, true)?;
                    return Ok(Event::Start(node));
                }
                quick_xml::events::Event::End(end) => {
                    let name = local_name(end.name().as_ref()).to_string();
                    self.depth = self.depth.saturating_sub(1);
                    self.open.pop();
                    return Ok(Event::End(name));
                }
                quick_xml::events::Event::Text(text) => {
                    let owning = text.xml10_content();
                    let trimmed = owning.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    return Ok(Event::Text(trimmed.to_string()));
                }
                quick_xml::events::Event::CData(cdata) => {
                    // CDATA is taken verbatim by design: it exists precisely so the
                    // author can write characters that would otherwise be markup.
                    let trimmed = cdata.into_inner().trim().to_string();
                    if trimmed.is_empty() {
                        continue;
                    }
                    return Ok(Event::Text(trimmed));
                }
                quick_xml::events::Event::Eof => {
                    // quick-xml reports a clean EOF for a truncated document, so
                    // the imbalance is detected here instead.
                    if let Some(unclosed) = self.open.last() {
                        return Err(OfdError::Xml(steleofd_error::XmlError::Syntax {
                            line: 0,
                            col: 0,
                            msg: format!("unexpected end of document, <{unclosed}> is not closed"),
                        }));
                    }
                    return Ok(Event::Eof);
                }
                // Declarations, comments, processing instructions and DTDs carry
                // no document data, so they are skipped rather than surfaced.
                _ => continue,
            }
        }
    }

    /// Skip to and including the matching end tag; a self-closing element is
    /// already complete, so this consumes nothing.
    pub fn skip_element(&mut self, node: &Node) -> Result<()> {
        if node.is_self_closing() {
            return Ok(());
        }
        let mut depth = 1usize;
        loop {
            match self.next_event()? {
                Event::Start(n) => {
                    if !n.is_self_closing() {
                        depth += 1;
                    }
                }
                Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Event::Eof => return Ok(()),
                Event::Text(_) => {}
            }
        }
    }

    /// Read the next event, requiring it to be a start tag.
    pub fn expect_start(&mut self, what: &str) -> Result<Node> {
        match self.next_event()? {
            Event::Start(node) => Ok(node),
            Event::End(name) => Err(OfdError::Xml(steleofd_error::XmlError::Syntax {
                line: 0,
                col: 0,
                msg: format!("expected <{what}>, found </{name}>"),
            })),
            Event::Text(t) => Err(OfdError::Xml(steleofd_error::XmlError::Syntax {
                line: 0,
                col: 0,
                msg: format!("expected <{what}>, found text {t:?}"),
            })),
            Event::Eof => Err(OfdError::Xml(steleofd_error::XmlError::Syntax {
                line: 0,
                col: 0,
                msg: format!("expected <{what}>, document ended"),
            })),
        }
    }

    /// Collect the character data of the element just opened.
    /// Used for leaf elements such as `<DocRoot>` whose value is their text.
    pub fn read_text(&mut self) -> Result<String> {
        let mut out = String::new();
        loop {
            match self.next_event()? {
                Event::Text(t) => out.push_str(&t),
                Event::End(_) => return Ok(out),
                Event::Start(nested) => {
                    // Nested markup is not text; skip it so a stray element cannot
                    // splice its contents into the parent's value.
                    self.skip_element(&nested)?;
                }
                Event::Eof => return Ok(out),
            }
        }
    }
}
