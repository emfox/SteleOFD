//! Font registry mapping ids to face data and origin.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use steleofd_error::{FontError, Result};

/// Font identifier, unique within one parsing session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FontId(pub u32);

/// Where a font came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSource {
    /// Embedded in the container, the most reproducible option.
    Embedded {
        /// Path inside the archive, for example `Doc_0/Res/Font_0.ttf`.
        path: String,
    },
    /// Resolved from the system by family name.
    System {
        /// Requested family name.
        family: String,
    },
    /// A face the document named and shipped no file for. Unlike
    /// [`Self::System`] it has not been found anywhere yet, so a reader can
    /// still tell "resolved from the host" from "asked for and missing".
    Unshipped {
        /// Family name the declaration named.
        family: String,
    },
    /// Unresolved; a substitute face will be used.
    Fallback {
        /// Family name that was requested.
        requested: String,
    },
}

/// What the document said about a face it did not ship. Clause 11.1 lists
/// these for exactly this purpose: matching a substitute against them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontTraits {
    /// Family name, when the declaration names one.
    pub family: Option<String>,
    /// Character class the face was declared for, such as `prc` or `big5`.
    pub charset: Option<String>,
    /// Whether the face is serifed.
    pub serif: Option<bool>,
    /// Whether the face is bold.
    pub bold: Option<bool>,
    /// Whether the face is italic.
    pub italic: Option<bool>,
    /// Whether the face is monospaced.
    pub fixed_width: Option<bool>,
}

impl FontTraits {
    /// Whether the declaration said nothing but the name.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// The family to search a host for, falling back to the declared name.
    pub fn search_family<'a>(&'a self, name: &'a str) -> &'a str {
        self.family.as_deref().unwrap_or(name)
    }

    /// Whether a substitute should be monospaced. Clause 11.1 makes
    /// `FixedWidth` optional, and a producer that names Courier or Mono has
    /// already said it by name, so the name answers when the flag is absent.
    pub fn wants_fixed_width(&self) -> bool {
        if let Some(declared) = self.fixed_width {
            return declared;
        }
        let name = self.family.as_deref().unwrap_or("").to_ascii_lowercase();
        const MARKERS: [&str; 8] = [
            "courier",
            "mono",
            "consolas",
            "menlo",
            "typewriter",
            "fixed",
            "code",
            "coding",
        ];
        MARKERS.iter().any(|marker| name.contains(marker))
    }
}

/// A registered font.
#[derive(Debug, Clone)]
pub struct FontEntry {
    /// Unique identifier.
    pub id: FontId,
    /// Family name.
    pub family: String,
    /// PostScript name when known.
    pub postscript_name: Option<String>,
    /// Where the face came from.
    pub source: FontSource,
    /// Face data, absent for system references not yet loaded.
    pub data: Option<Arc<Vec<u8>>>,
    /// Whether the face covers CJK.
    pub is_cjk: bool,
    /// What the declaration said, when it named a face it did not ship.
    pub traits: Option<FontTraits>,
    /// Whether this entry stands in for a face no host face was found for.
    pub unresolved: bool,
}

impl FontEntry {
    /// The family to ask a host for: the declared family name when there is
    /// one, otherwise the name the document gave the face.
    pub fn search_family(&self) -> &str {
        match &self.traits {
            Some(traits) => traits.search_family(&self.family),
            None => &self.family,
        }
    }

    /// What the document named, for a message a person reads.
    pub fn described_name(&self) -> &str {
        if self.family.is_empty() {
            "unnamed"
        } else {
            &self.family
        }
    }
}

/// A face a document asked for and the reader could not draw with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unresolved {
    /// The id the display list uses for it.
    pub id: FontId,
    /// The name the document gave the face.
    pub family: String,
    /// What the declaration said about it, when it said anything.
    pub traits: Option<FontTraits>,
    /// How the package presented it: embedded with unreadable bytes, or only
    /// named. A named face is the case a host search is meant to cover.
    pub source: FontSource,
}

/// Deduplicating registry of fonts.
/// The same binary is often referenced by both `DocumentRes.xml` and
/// `PublicRes.xml`; content hashing keeps a single copy in memory.
#[derive(Debug, Default)]
pub struct FontRegistry {
    entries: Vec<FontEntry>,
    dedup: std::collections::HashMap<u64, FontId>,
    by_family: std::collections::HashMap<String, FontId>,
}

impl FontRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an embedded face, reusing the id when the bytes are identical.
    pub fn register_embedded(
        &mut self,
        path: impl Into<String>,
        family: impl Into<String>,
        data: Arc<Vec<u8>>,
    ) -> FontId {
        let hash = fnv1a(&data);
        if let Some(&id) = self.dedup.get(&hash) {
            return id;
        }
        let family = family.into();
        let id = FontId(self.entries.len() as u32);
        self.entries.push(FontEntry {
            id,
            family: family.clone(),
            postscript_name: None,
            source: FontSource::Embedded { path: path.into() },
            is_cjk: detect_cjk(&data),
            data: Some(data),
            traits: None,
            unresolved: false,
        });
        self.dedup.insert(hash, id);
        self.by_family.entry(family).or_insert(id);
        id
    }

    /// Register a system font reference; the manager loads bytes on demand.
    pub fn register_system(&mut self, family: impl Into<String>) -> FontId {
        let family = family.into();
        if let Some(&id) = self.by_family.get(&family) {
            return id;
        }
        let id = FontId(self.entries.len() as u32);
        self.entries.push(FontEntry {
            id,
            family: family.clone(),
            postscript_name: None,
            source: FontSource::System {
                family: family.clone(),
            },
            is_cjk: false,
            data: None,
            traits: None,
            unresolved: false,
        });
        self.by_family.insert(family, id);
        id
    }

    /// Register a face the document named but did not ship, with the clause
    /// 11.1 traits a host search matches on. Its source stays `Unshipped`, not
    /// `System`, because nothing has been found for it yet.
    pub fn register_unshipped(&mut self, family: impl Into<String>, traits: FontTraits) -> FontId {
        let family = family.into();
        // Two declarations of one family that agree on everything a match can
        // see are the same reference; one that differs in weight or slope is
        // not, and gets its own id.
        if let Some(&id) = self.by_family.get(&family)
            && let Ok(entry) = self.get(id)
            && entry.traits.as_ref() == Some(&traits)
        {
            return id;
        }
        let id = FontId(self.entries.len() as u32);
        self.entries.push(FontEntry {
            id,
            family: family.clone(),
            postscript_name: None,
            source: FontSource::Unshipped {
                family: traits.search_family(&family).to_string(),
            },
            is_cjk: false,
            data: None,
            traits: Some(traits),
            unresolved: false,
        });
        self.by_family.entry(family).or_insert(id);
        id
    }

    /// Register a substitute face, resolved from `default_cjk_family` when it
    /// is first loaded. Used to cover characters no embedded face carries.
    pub fn register_fallback(&mut self, family: impl Into<String>) -> FontId {
        self.register_fallback_with_data(family, None)
    }

    /// Register a substitute face that carries its own bytes, so the chain can
    /// cover a script even on a host with no system font for it. A caller that
    /// ships a known face pins it; a `None` defers to `default_cjk_family`.
    pub fn register_fallback_with_data(
        &mut self,
        family: impl Into<String>,
        data: Option<Arc<Vec<u8>>>,
    ) -> FontId {
        let family = family.into();
        if let Some(&id) = self.by_family.get(&family) {
            return id;
        }
        let is_cjk = match &data {
            Some(bytes) => detect_cjk(bytes),
            None => true,
        };
        let id = FontId(self.entries.len() as u32);
        self.entries.push(FontEntry {
            id,
            family: family.clone(),
            postscript_name: None,
            source: FontSource::Fallback {
                requested: family.clone(),
            },
            is_cjk,
            data,
            // The engine's own substitute is a bare family name — it is often
            // the literal "sans-serif" — so an empty trait set is what a host
            // search should see, not a set of guesses.
            traits: Some(FontTraits::default()),
            unresolved: false,
        });
        self.by_family.insert(family, id);
        id
    }

    /// Look up an entry by id.
    pub fn get(&self, id: FontId) -> Result<&FontEntry> {
        self.entries
            .get(id.0 as usize)
            .ok_or_else(|| FontError::Parse(format!("FontId({}) does not exist", id.0)).into())
    }

    /// Number of registered fonts.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry holds no fonts.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all fonts, for missing-font reporting.
    pub fn iter(&self) -> impl Iterator<Item = &FontEntry> {
        self.entries.iter()
    }

    /// Families with no usable bytes yet, embedded or resolved. A reference
    /// the manager has given up on is excluded; see [`Self::unresolved`].
    pub fn missing_families(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|e| e.data.is_none() && !e.unresolved)
            .map(|e| e.family.as_str())
            .collect()
    }

    /// What a document asked for and did not get, one record per id. A face
    /// the package never shipped is otherwise invisible: a blank run and a
    /// drawn one look alike in a PNG.
    pub fn unresolved(&self) -> Vec<Unresolved> {
        self.entries
            .iter()
            .filter(|e| e.unresolved)
            .map(|e| Unresolved {
                id: e.id,
                family: e.described_name().to_string(),
                traits: e.traits.clone(),
                source: e.source.clone(),
            })
            .collect()
    }
}

/// FNV-1a 64-bit, fast and sufficient for deduplication.
fn fnv1a(data: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// Rough CJK detection by looking for the UTF-16BE bytes of U+4E2D.
fn detect_cjk(data: &[u8]) -> bool {
    const CJK_MARKER: &[u8] = &[0x4E, 0x2D];
    data.windows(2).any(|w| w == CJK_MARKER)
}
