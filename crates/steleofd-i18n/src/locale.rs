//! Locale identifiers and negotiation.
//! BCP 47 tags such as `zh-CN` and `en-US`, parsed by `unic-langid`, with
//! negotiation from user preference to the closest available locale.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::BUILTIN_LOCALES;

/// A BCP 47 language tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Locale(String);

impl Locale {
    /// Parse a tag, falling back to the default when malformed.
    pub fn new(tag: &str) -> Self {
        match unic_langid::LanguageIdentifier::from_str(tag) {
            Ok(id) => Self(id.to_string()),
            Err(_) => {
                tracing::warn!(tag, "invalid language tag, falling back to default");
                Self(crate::FALLBACK_LOCALE.to_string())
            }
        }
    }

    /// Simplified Chinese.
    pub fn zh_cn() -> Self {
        Self("zh-CN".to_string())
    }

    /// English, United States.
    pub fn en_us() -> Self {
        Self("en-US".to_string())
    }

    /// The fallback locale.
    pub fn fallback() -> Self {
        Self(crate::FALLBACK_LOCALE.to_string())
    }

    /// The full tag.
    pub fn tag(&self) -> &str {
        &self.0
    }

    /// Language subtag only, so `zh-CN` yields `zh`.
    pub fn language(&self) -> &str {
        self.0.split('-').next().unwrap_or(&self.0)
    }

    /// Whether the language runs right to left, which UI layout must mirror.
    pub fn is_rtl(&self) -> bool {
        matches!(self.language(), "ar" | "he" | "fa" | "ur")
    }

    /// Whether the language is CJK, affecting font choice and line breaking.
    pub fn is_cjk(&self) -> bool {
        matches!(self.language(), "zh" | "ja" | "ko")
    }

    /// Whether this locale is compiled into the current build.
    pub fn is_builtin(&self) -> bool {
        BUILTIN_LOCALES.contains(&self.0.as_str())
    }
}

impl Default for Locale {
    fn default() -> Self {
        Self::fallback()
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Locale {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

/// How a locale is chosen.
#[derive(Debug, Clone, Default)]
pub enum LocaleNegotiation {
    /// Use an explicit locale, ignoring the system.
    Fixed(Locale),
    /// Follow the system, defaulting when detection fails.
    #[default]
    System,
    /// Walk a preference list, as with HTTP `Accept-Language`.
    PreferenceList(Vec<Locale>),
}

impl LocaleNegotiation {
    /// Pick the best available locale.
    /// Matching runs exact tag, then same language ignoring region, then fallback:
    /// with `{zh-CN, en-US}` available, `zh-TW` resolves to `zh-CN`.
    pub fn negotiate(&self, available: &[Locale]) -> Locale {
        let candidates: Vec<Locale> = match self {
            Self::Fixed(l) => vec![l.clone()],
            Self::PreferenceList(list) => list.clone(),
            Self::System => vec![detect_system_locale()],
        };

        for wanted in &candidates {
            if let Some(l) = available.iter().find(|a| a.0 == wanted.0) {
                return l.clone();
            }
            if let Some(l) = available.iter().find(|a| a.language() == wanted.language()) {
                return l.clone();
            }
        }

        available
            .iter()
            .find(|a| a.0 == crate::FALLBACK_LOCALE)
            .cloned()
            .unwrap_or_else(Locale::fallback)
    }
}

/// Detect the system locale from environment variables.
#[cfg(not(target_arch = "wasm32"))]
fn detect_system_locale() -> Locale {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(v) = std::env::var(key)
            && !v.is_empty()
            && v != "C"
            && v != "POSIX"
        {
            let tag = v.split('.').next().unwrap_or(&v).replace('_', "-");
            return Locale::new(&tag);
        }
    }
    Locale::fallback()
}

/// On WASM the host supplies the locale explicitly.
#[cfg(target_arch = "wasm32")]
fn detect_system_locale() -> Locale {
    Locale::fallback()
}
