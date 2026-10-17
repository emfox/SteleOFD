//! Loading, caching and fallback of font faces.
//! Queries return `Arc<FontFace>` rather than a borrow, because extending one
//! with `unsafe` dangled once the cache was cleared.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use steleofd_error::{FontError, Result};
use steleofd_geometry::Path;

use crate::face::FontFace;
use crate::registry::{FontId, FontRegistry, FontSource, Unresolved};

/// Owns the registry and the cache of loaded faces. The system source is a
/// thread-local: font-kit's fontconfig pointer is neither Send nor Sync.
pub struct FontManager {
    registry: FontRegistry,
    faces: RwLock<HashMap<FontId, Arc<FontFace>>>,
    fallback_chain: RwLock<Vec<FontId>>,
    default_cjk_family: RwLock<String>,
    /// Ids a host search already came back empty for. Asking again cannot
    /// change the answer, and a glyph loop would ask once per character.
    unresolved: RwLock<HashSet<FontId>>,
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
            unresolved: RwLock::new(HashSet::new()),
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
        self.face_for(id, &[])
    }

    /// [`Self::face`], told what the caller is about to draw: the code points
    /// are what separates a substitute that can draw the run from one that
    /// merely exists. An empty slice searches as before.
    pub fn face_for(&self, id: FontId, wanted: &[u32]) -> Result<Arc<FontFace>> {
        // A face already loaded may be one a previous caller accepted without
        // knowing the text, so a caller that knows it checks coverage again
        // rather than trusting the cached answer for a different string.
        if let Some(f) = self.faces.read().ok().and_then(|m| m.get(&id).cloned()) {
            if wanted.is_empty() || f.covers(wanted) {
                return Ok(f);
            }
            self.note_unresolved(id);
            return Err(FontError::Missing {
                family: f.family().to_string(),
            }
            .into());
        }
        // A search that already failed is not repeated: the answer cannot
        // change, and a glyph loop asks once per character.
        if self.unresolved.read().is_ok_and(|set| set.contains(&id)) {
            let family = self
                .registry
                .get(id)
                .map(|e| e.described_name().to_string())
                .unwrap_or_default();
            return Err(FontError::Missing { family }.into());
        }

        let entry = self.registry.get(id)?.clone();
        let data = match &entry.source {
            FontSource::Embedded { .. } => match entry.data.clone() {
                Some(d) => d,
                None => {
                    self.note_unresolved(id);
                    return Err(FontError::Missing {
                        family: entry.described_name().to_string(),
                    }
                    .into());
                }
            },
            // A substitute that carries none of the characters is no
            // substitute: the text drops either way, and "drawn with a
            // substitute" over a blank page is a worse lie than a warning.
            FontSource::System { .. } => match self.find_system_font(&entry, wanted)? {
                Some(data) if wanted.is_empty() || Self::covers(&data, wanted) => data,
                _ => {
                    self.note_unresolved(id);
                    return Err(FontError::Missing {
                        family: entry.described_name().to_string(),
                    }
                    .into());
                }
            },
            // The clause 11.1 traits are what a host search matches on, so an
            // unshipped declaration is searched like a system reference.
            FontSource::Unshipped { .. } => match self.find_system_font(&entry, wanted)? {
                Some(data) if wanted.is_empty() || Self::covers(&data, wanted) => data,
                _ => {
                    self.note_unresolved(id);
                    return Err(FontError::Missing {
                        family: entry.described_name().to_string(),
                    }
                    .into());
                }
            },
            // The engine's own substitute starts from the pinned CJK family
            // and only takes a generic when this host cannot answer with it.
            FontSource::Fallback { .. } => match entry.data.clone() {
                Some(data) => data,
                None => match self.find_system_font(&entry, wanted)? {
                    Some(data) => data,
                    None => {
                        self.note_unresolved(id);
                        return Err(FontError::Missing {
                            family: entry.described_name().to_string(),
                        }
                        .into());
                    }
                },
            },
        };
        let is_fallback = matches!(entry.source, FontSource::Fallback { .. });

        let face = Arc::new(FontFace::new(id, entry.family.clone(), data, is_fallback)?);

        let mut faces = self
            .faces
            .write()
            .map_err(|_| FontError::Parse("font cache lock poisoned".to_string()))?;
        let stored = faces.entry(id).or_insert_with(|| face.clone()).clone();
        Ok(stored)
    }

    /// Remember that a host search for this id came back empty, so it is not
    /// repeated and a caller can report it.
    fn note_unresolved(&self, id: FontId) {
        if let Ok(mut set) = self.unresolved.write() {
            set.insert(id);
        }
    }

    /// Faces a document asked for and this build could not supply.
    pub fn unresolved(&self) -> Vec<Unresolved> {
        let Ok(set) = self.unresolved.read() else {
            return Vec::new();
        };
        self.registry
            .iter()
            .filter(|e| set.contains(&e.id))
            .map(|e| Unresolved {
                id: e.id,
                family: e.described_name().to_string(),
                traits: e.traits.clone(),
                source: e.source.clone(),
            })
            .collect()
    }

    /// Whether a face could not be supplied. Cheaper than [`Self::unresolved`]
    /// for a caller that only wants to know whether to warn.
    pub fn is_unresolved(&self, id: FontId) -> bool {
        self.unresolved.read().is_ok_and(|set| set.contains(&id))
    }

    /// Whether a face carries every code point given, read straight from its
    /// bytes so the question can be asked before a face is cached for an id.
    /// With no search compiled in nothing a host was asked for can be covered.
    fn covers(data: &Arc<Vec<u8>>, wanted: &[u32]) -> bool {
        FontFace::new(FontId(0), String::new(), data.clone(), false)
            .is_ok_and(|face| face.covers(wanted))
    }

    /// Find a host face for a reference the package only named, using the
    /// clause 11.1 traits as the match keys: name, then weight and slope, then
    /// a generic family of the declared serif style or fixed width.
    #[cfg(all(feature = "system-fonts", not(target_arch = "wasm32")))]
    fn find_system_font(
        &self,
        entry: &crate::registry::FontEntry,
        wanted: &[u32],
    ) -> Result<Option<Arc<Vec<u8>>>> {
        use font_kit::handle::Handle;
        use font_kit::properties::{Properties, Style, Weight};

        let traits = entry.traits.clone().unwrap_or_default();
        let mut properties = Properties::new();
        if traits.bold == Some(true) {
            properties.weight = Weight::BOLD;
        }
        if traits.italic == Some(true) {
            properties.style = Style::Italic;
        }

        thread_local! {
            static SYSTEM: font_kit::source::SystemSource = font_kit::source::SystemSource::new();
        }
        SYSTEM.with(|source| {
            // Every name the declaration offers: the search family, the
            // localised one, then the writer's spelling of the face name.
            let mut candidates: Vec<String> = vec![entry.search_family().to_string()];
            if let Some(traits) = &entry.traits
                && let Some(name) = &traits.family
            {
                candidates.push(name.clone());
            }
            if !entry.family.is_empty() {
                candidates.push(entry.family.clone());
            }
            // A name can be offered twice — the search family and the entry
            // family are often the same string — and `dedup` would keep both
            // when they are not adjacent.
            let mut seen: Vec<String> = Vec::new();
            candidates.retain(|name| {
                let key = name.to_lowercase();
                let fresh = !seen.contains(&key);
                seen.push(key);
                fresh
            });
            let known: Vec<String> = source
                .all_families()
                .unwrap_or_default()
                .into_iter()
                .map(|name| name.to_lowercase())
                .collect();
            for family in &candidates {
                // Only an exact listing counts: a substring test lets "serif"
                // select "Noto Serif CJK SC", a face never asked for.
                if !known.contains(&family.to_lowercase()) {
                    continue;
                }
                if let Ok(handle) = source.select_family_by_name(family)
                    && let Some(data) = load_first(&handle, &properties)?
                    && (wanted.is_empty() || Self::covers(&data, wanted))
                {
                    return Ok(Some(data));
                }
            }

            // The generic step: the shape clause 11.1's `Serif` and
            // `FixedWidth` ask for. A generic is a shape, not cover coverage,
            // so each is taken only if it carries the caller's code points.
            let mut order = Vec::with_capacity(3);
            match (traits.fixed_width, traits.serif) {
                (Some(true), _) => order.extend([
                    font_kit::family_name::FamilyName::Monospace,
                    font_kit::family_name::FamilyName::Serif,
                    font_kit::family_name::FamilyName::SansSerif,
                ]),
                (_, Some(true)) => order.extend([
                    font_kit::family_name::FamilyName::Serif,
                    font_kit::family_name::FamilyName::SansSerif,
                    font_kit::family_name::FamilyName::Monospace,
                ]),
                _ => order.extend([
                    font_kit::family_name::FamilyName::SansSerif,
                    font_kit::family_name::FamilyName::Serif,
                    font_kit::family_name::FamilyName::Monospace,
                ]),
            }
            // Hosts map their generic families differently, so the pinned
            // default is asked for by name before any generic: that is what
            // keeps `cjk-substitute` stable across machines.
            let pinned = self
                .default_cjk_family
                .read()
                .map(|s| s.clone())
                .unwrap_or_else(|_| "Noto Sans CJK SC".to_string());
            let pinned_key = pinned.to_lowercase();
            let pinned_data = if known.contains(&pinned_key) {
                source
                    .select_family_by_name(&pinned)
                    .ok()
                    .map(|h| load_first(&h, &properties))
                    .transpose()?
                    .flatten()
            } else {
                None
            };
            if let Some(data) = pinned_data.as_ref()
                && (wanted.is_empty() || Self::covers(data, wanted))
            {
                return Ok(Some(data.clone()));
            }

            // The default family, when the host has it, is the shape-neutral
            // answer this fallback most wants; the generic families come
            // after it and only replace it when they cover more.
            let mut fallback = pinned_data;
            for generic in order {
                let Ok(handle) = source.select_best_match(&[generic], &properties) else {
                    continue;
                };
                let data = match handle {
                    Handle::Path { path, .. } => Arc::new(std::fs::read(path)?),
                    Handle::Memory { bytes, .. } => Arc::new(bytes.as_ref().clone()),
                };
                if wanted.is_empty() || Self::covers(&data, wanted) {
                    return Ok(Some(data));
                }
                // Coverage often lives in a family beside the generic — a
                // Latin default for `sans-serif`, `Noto Sans CJK SC` next to
                // it — so those are asked for by name before the shape alone.
                if let Some(data) = covering_family(source, &known, wanted, &properties)? {
                    return Ok(Some(data));
                }
                fallback.get_or_insert(data);
            }
            Ok(fallback)
        })
    }

    /// Find a face in the fallback chain that covers a code point.
    pub fn fallback_for(&self, codepoint: u32) -> Option<(FontId, Arc<FontFace>)> {
        self.fallback_for_wanted(codepoint, &[codepoint])
    }

    /// [`Self::fallback_for`], with the code points of the run to choose by;
    /// a member that covers them all beats one that merely carries this code.
    /// Coverage is read from the faces, not from whether a search found them.
    pub fn fallback_for_wanted(
        &self,
        codepoint: u32,
        wanted: &[u32],
    ) -> Option<(FontId, Arc<FontFace>)> {
        let chain = self.fallback_chain.read().ok()?.clone();
        let mut first: Option<(FontId, Arc<FontFace>)> = None;
        for id in chain {
            if let Ok(face) = self.face(id)
                && face.glyph_id(codepoint).is_some()
            {
                if wanted.len() > 1 && !face.covers(wanted) {
                    first.get_or_insert((id, face));
                    continue;
                }
                return Some((id, face));
            }
        }
        first
    }

    /// The outline for one run code and the face it came from, substituting
    /// from the fallback chain when the run's own face lacks it. A code already
    /// resolved to a glyph id is never substituted.
    pub fn outline_for_code(
        &self,
        face: &FontFace,
        code: u16,
        glyph_index: bool,
    ) -> Option<(Path, Arc<FontFace>)> {
        self.outline_for_code_in(face, code, glyph_index, &[])
    }

    /// [`Self::outline_for_code`], with the code points of the run in hand, so
    /// a substitute is chosen for what it can cover rather than for existing.
    pub fn outline_for_code_in(
        &self,
        face: &FontFace,
        code: u16,
        glyph_index: bool,
        run: &[u32],
    ) -> Option<(Path, Arc<FontFace>)> {
        let run_hint = u32::from(code);
        // A run of characters describes itself; a run of glyph indices says
        // nothing, so the one code being asked about stands in for it.
        let wanted = if run.is_empty() {
            std::slice::from_ref(&run_hint)
        } else {
            run
        };
        if glyph_index {
            let outline = face.outline_for_run_code(code, true)?;
            return Some((outline, self.face(face.id()).ok()?));
        }
        if let Some(outline) = face.outline_for_run_code(code, false) {
            return Some((outline, self.face(face.id()).ok()?));
        }
        let (_, alt) = self.fallback_for_wanted(run_hint, wanted)?;
        let outline = alt.outline_for_run_code(code, false)?;
        Some((outline, alt))
    }

    /// Replace the fallback chain.
    pub fn set_fallback_chain(&self, ids: Vec<FontId>) {
        if let Ok(mut chain) = self.fallback_chain.write() {
            *chain = ids;
        }
    }

    /// Build the fallback chain from what the registry holds and install it.
    /// Embedded faces go in first, CJK then the rest, each group in id order.
    /// A system reference joins only when this build can resolve one.
    pub fn build_fallback_chain(&mut self) -> Vec<FontId> {
        let mut wide: Vec<FontId> = Vec::new();
        let mut pinned: Vec<FontId> = Vec::new();
        let mut narrow: Vec<FontId> = Vec::new();
        let resolvable = cfg!(all(feature = "system-fonts", not(target_arch = "wasm32")));
        for entry in self.registry.iter() {
            match &entry.source {
                FontSource::Embedded { .. } => {
                    if entry.is_cjk {
                        wide.push(entry.id);
                    } else {
                        narrow.push(entry.id);
                    }
                }
                // A fallback that carries its own bytes is known to load, so it
                // is consulted before an embedded wide face and long before a
                // system-resolved substitute that may not exist on this host.
                FontSource::Fallback { .. } if entry.data.is_some() => pinned.push(entry.id),
                // A declaration the package did not ship a file for is a face
                // the host may have under the name the author asked for, so it
                // is tried before the engine's own blanket substitute.
                FontSource::Unshipped { .. } => wide.push(entry.id),
                FontSource::Fallback { .. } => wide.push(entry.id),
                FontSource::System { .. } if resolvable => narrow.push(entry.id),
                _ => {}
            }
        }

        // A document that ships no wide face and names none leaves its CJK
        // text with nothing to fall back to, and the page renders blank rather
        // than wrong. Chain one substitute, loaded only if a code needs it.
        if wide.is_empty() && pinned.is_empty() && resolvable {
            let id = self.registry.register_fallback("cjk-substitute");
            wide.push(id);
        }

        // Ids are handed out in registration order, so sorting only guards
        // against a caller that built a registry some other way.
        wide.sort_unstable();
        pinned.sort_unstable();
        narrow.sort_unstable();
        pinned.extend(wide);
        pinned.extend(narrow);
        pinned.dedup();
        self.set_fallback_chain(pinned.clone());
        pinned
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

    /// No host search is compiled in — the feature is off, or the target has
    /// no system fonts to resolve — so nothing named can be found.
    #[cfg(not(all(feature = "system-fonts", not(target_arch = "wasm32"))))]
    fn find_system_font(
        &self,
        _entry: &crate::registry::FontEntry,
        _wanted: &[u32],
    ) -> Result<Option<Arc<Vec<u8>>>> {
        Ok(None)
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}

/// A listed family that carries every code point given, in the order the host
/// lists them. The generic families are aliases a host maps once, so they
/// answer with the Latin default even when a face next to it has the script.
#[cfg(all(feature = "system-fonts", not(target_arch = "wasm32")))]
fn covering_family(
    source: &font_kit::source::SystemSource,
    known: &[String],
    wanted: &[u32],
    properties: &font_kit::properties::Properties,
) -> Result<Option<Arc<Vec<u8>>>> {
    for name in known {
        let Ok(handle) = source.select_family_by_name(name) else {
            continue;
        };
        if let Some(data) = load_first(&handle, properties)?
            && FontManager::covers(&data, wanted)
        {
            return Ok(Some(data));
        }
    }
    Ok(None)
}

/// The first face of a family that matches the declared weight and slope, or
/// its first face when none does. A family often ships several faces and the
/// declaration only asks for a shape.
#[cfg(all(feature = "system-fonts", not(target_arch = "wasm32")))]
fn load_first(
    family: &font_kit::family_handle::FamilyHandle,
    properties: &font_kit::properties::Properties,
) -> Result<Option<Arc<Vec<u8>>>> {
    use font_kit::handle::Handle;

    let handles = family.fonts();
    let mut best: Option<usize> = None;
    for (index, handle) in handles.iter().enumerate() {
        let (path, font_index) = match handle {
            Handle::Path { path, font_index } => (path.clone(), *font_index),
            Handle::Memory { .. } => continue,
        };
        let Ok(font) = font_kit::font::Font::from_path(&path, font_index) else {
            continue;
        };
        let weight = font.properties().weight;
        let style = font.properties().style;
        let weight_ok = (weight.0 - properties.weight.0).abs() < 100.0;
        let style_ok = style == properties.style;
        if weight_ok && style_ok {
            best = Some(index);
            break;
        }
        if best.is_none() {
            best = Some(index);
        }
    }
    let Some(index) = best else {
        return Ok(None);
    };
    Ok(match &handles[index] {
        Handle::Path { path, .. } => Some(Arc::new(std::fs::read(path)?)),
        Handle::Memory { bytes, .. } => Some(Arc::new(bytes.as_ref().clone())),
    })
}
