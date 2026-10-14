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
    /// Unresolved; a substitute face will be used.
    Fallback {
        /// Family name that was requested.
        requested: String,
    },
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

    /// Families with no usable data, checked before conversion to avoid garbled output.
    pub fn missing_families(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|e| matches!(e.source, FontSource::Fallback { .. }) || e.data.is_none())
            .map(|e| e.family.as_str())
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
