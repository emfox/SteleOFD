//! Loading, caching and fallback of font faces.
//! Queries return `Arc<FontFace>` rather than a borrow, because extending one
//! with `unsafe` dangled once the cache was cleared.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use steleofd_error::{FontError, Result};

use crate::face::FontFace;
use crate::registry::{FontId, FontRegistry, FontSource};

/// Owns the registry and the cache of loaded faces. The system source is a
/// thread-local: font-kit's fontconfig pointer is neither Send nor Sync.
pub struct FontManager {
    registry: FontRegistry,
    faces: RwLock<HashMap<FontId, Arc<FontFace>>>,
    fallback_chain: RwLock<Vec<FontId>>,
    default_cjk_family: RwLock<String>,
}

impl std::fmt::Debug for FontManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontManager")
            .field("registered", &self.registry.len())
            .field("loaded", &self.faces.read().map(|m| m.len()).unwrap_or(0))
            .finish()
    }
}

impl FontManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self {
            registry: FontRegistry::new(),
            faces: RwLock::new(HashMap::new()),
            fallback_chain: RwLock::new(Vec::new()),
            default_cjk_family: RwLock::new("Noto Sans CJK SC".to_string()),
        }
    }

    /// Mutable registry access, used by the parser to register font references.
    pub fn registry_mut(&mut self) -> &mut FontRegistry {
        &mut self.registry
    }

    /// Read-only registry access.
    pub fn registry(&self) -> &FontRegistry {
        &self.registry
    }

    /// Load a face if needed and hand back a shared handle.
    pub fn face(&self, id: FontId) -> Result<Arc<FontFace>> {
        if let Some(f) = self.faces.read().ok().and_then(|m| m.get(&id).cloned()) {
            return Ok(f);
        }

        let entry = self.registry.get(id)?.clone();
        let (data, is_fallback) = match &entry.source {
            FontSource::Embedded { .. } => {
                let d = entry.data.clone().ok_or_else(|| FontError::Missing {
                    family: entry.family.clone(),
                })?;
                (d, false)
            }
            FontSource::System { family } => (self.load_system_font(family)?, false),
            FontSource::Fallback { .. } => {
                let family = self
                    .default_cjk_family
                    .read()
                    .map(|s| s.clone())
                    .unwrap_or_else(|_| "Noto Sans CJK SC".to_string());
                (self.load_system_font(&family)?, true)
            }
        };

        let face = Arc::new(FontFace::new(id, entry.family.clone(), data, is_fallback)?);

        let mut faces = self
            .faces
            .write()
            .map_err(|_| FontError::Parse("font cache lock poisoned".to_string()))?;
        let stored = faces.entry(id).or_insert_with(|| face.clone()).clone();
        Ok(stored)
    }

    /// Find a face in the fallback chain that covers a code point.
    pub fn fallback_for(&self, codepoint: u32) -> Option<(FontId, Arc<FontFace>)> {
        let chain = self.fallback_chain.read().ok()?.clone();
        for id in chain {
            if let Ok(face) = self.face(id)
                && face.glyph_id(codepoint).is_some()
            {
                return Some((id, face));
            }
        }
        None
    }

    /// Replace the fallback chain.
    pub fn set_fallback_chain(&self, ids: Vec<FontId>) {
        if let Ok(mut chain) = self.fallback_chain.write() {
            *chain = ids;
        }
    }

    /// Current fallback chain.
    pub fn fallback_chain(&self) -> Vec<FontId> {
        self.fallback_chain
            .read()
            .map(|c| c.clone())
            .unwrap_or_default()
    }

    /// Set the family used when CJK text needs a substitute.
    pub fn set_default_cjk_family(&self, family: impl Into<String>) {
        if let Ok(mut f) = self.default_cjk_family.write() {
            *f = family.into();
        }
    }

    /// Number of faces currently cached.
    pub fn loaded_count(&self) -> usize {
        self.faces.read().map(|m| m.len()).unwrap_or(0)
    }

    /// Drop the cache to reclaim memory when switching documents.
    ///
    /// Handles already held by callers stay valid.
    pub fn clear_cache(&self) {
        if let Ok(mut m) = self.faces.write() {
            m.clear();
        }
    }

    /// Load a face from the system by family name.
    #[cfg(all(feature = "system-fonts", not(target_arch = "wasm32")))]
    fn load_system_font(&self, family: &str) -> Result<Arc<Vec<u8>>> {
        use font_kit::family_name::FamilyName;
        use font_kit::handle::Handle;
        use font_kit::properties::Properties;

        // One source per thread: fontconfig configs are per-thread anyway, and
        // this keeps the raw pointer inside font-kit off the shared struct.
        thread_local! {
            static SYSTEM: font_kit::source::SystemSource = font_kit::source::SystemSource::new();
        }
        SYSTEM.with(|source| {
            let handle = source
                .select_best_match(&[FamilyName::Title(family.to_string())], &Properties::new())
                .map_err(|_| FontError::Missing {
                    family: family.to_string(),
                })?;
            match handle {
                Handle::Path { path, .. } => Ok(Arc::new(std::fs::read(path)?)),
                Handle::Memory { bytes, .. } => Ok(Arc::new((*bytes).clone())),
            }
        })
    }

    /// Degraded path when system font lookup is compiled out: either the
    /// feature is off, or the target has no system fonts to resolve.
    #[cfg(not(all(feature = "system-fonts", not(target_arch = "wasm32"))))]
    fn load_system_font(&self, family: &str) -> Result<Arc<Vec<u8>>> {
        Err(FontError::Missing {
            family: family.to_string(),
        }
        .into())
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}
