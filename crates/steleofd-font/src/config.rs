//! Which families a host is searched for, as data rather than as code. The
//! engine ships a default, a caller may replace or extend it, and the order is
//! the answer: the first listed family that carries the text wins.

use std::ops::Deref;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Families to search, in order. Shared because a [`FontConfig`] is cloned
/// into a manager and then held for the life of a document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FamilyList(Arc<Vec<String>>);

impl FamilyList {
    /// A list from anything naming families, in the order given.
    pub fn new<I, S>(families: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut list: Vec<String> = Vec::new();
        for family in families {
            let family = family.into();
            let name = family.trim();
            // A host compares names case-insensitively, so a list that offers
            // one twice would only cost a lookup; blank names cost a scan.
            if name.is_empty()
                || list
                    .iter()
                    .any(|seen: &String| seen.eq_ignore_ascii_case(name))
            {
                continue;
            }
            list.push(name.to_string());
        }
        Self(Arc::new(list))
    }

    /// Whether the list names no family at all.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The same list with `family` ahead of everything else, once.
    pub fn prepend(&self, family: &str) -> Self {
        let mut families = vec![family.to_string()];
        families.extend(self.0.iter().cloned());
        Self::new(families)
    }
}

impl Deref for FamilyList {
    type Target = [String];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&FamilyList> for Vec<String> {
    fn from(list: &FamilyList) -> Self {
        list.0.as_ref().clone()
    }
}

/// What the engine needs to know to look for a face on this host. An empty
/// field falls back to the built-in list, so the default is the shipped policy
/// and a caller only names what it wants to change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontConfig {
    /// Families searched first when CJK text needs a substitute.
    #[serde(default, skip_serializing_if = "FamilyList::is_empty")]
    pub cjk_families: FamilyList,
    /// Family substituted for a character no other face carries.
    #[serde(default, skip_serializing_if = "FamilyList::is_empty")]
    pub fallback_families: FamilyList,
}

impl FontConfig {
    /// A configuration that names its own families.
    pub fn new<I, S, J, T>(cjk_families: I, fallback_families: J) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
        J: IntoIterator<Item = T>,
        T: Into<String>,
    {
        Self {
            cjk_families: FamilyList::new(cjk_families),
            fallback_families: FamilyList::new(fallback_families),
        }
    }

    /// The families to search, as one ordered answer: the CJK list, the
    /// built-in list, then the fallback families. A configured list leads but
    /// never replaces what the engine ships.
    pub fn search_order(&self) -> FamilyList {
        let mut families: Vec<String> = Vec::new();
        families.extend(self.cjk_families.iter().cloned());
        families.extend(default_cjk_families().iter().cloned());
        families.extend(self.fallback_families.iter().cloned());
        FamilyList::new(families)
    }

    /// Whether nothing was configured, so the built-in list is in force.
    pub fn uses_default_cjk_families(&self) -> bool {
        self.cjk_families.is_empty()
    }

    /// Fill in what was left out from `other`, for a caller layering a
    /// document's preference over a host's or a user's.
    pub fn with_fallback_from(mut self, other: &Self) -> Self {
        if self.cjk_families.is_empty() {
            self.cjk_families = other.cjk_families.clone();
        }
        if self.fallback_families.is_empty() {
            self.fallback_families = other.fallback_families.clone();
        }
        self
    }
}

/// The families the engine looks for when nothing else says. The order is the
/// policy: the glyph shapes the pinned Noto face draws, then the faces each
/// host keeps Chinese in. A host with none of them is still scanned in full.
pub fn default_cjk_families() -> FamilyList {
    FamilyList::new([
        "Noto Sans CJK SC",
        "Source Han Sans SC",
        "Microsoft YaHei",
        "SimSun",
        "PingFang SC",
        "Hiragino Sans GB",
        "WenQuanYi Zen Hei",
        "Droid Sans Fallback",
    ])
}
