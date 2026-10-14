//! Fluent message bundles keyed by locale.

use std::collections::HashMap;
use std::sync::Arc;

use fluent::{FluentArgs, FluentResource};
use fluent_bundle::concurrent::FluentBundle as ConcurrentFluentBundle;

use crate::BUILTIN_LOCALES;
use crate::locale::{Locale, LocaleNegotiation};
use crate::messages::MessageId;

// Catalogues are embedded at compile time: no runtime file scanning on desktop,
// no fetch on WASM, and a missing file fails the build rather than a user.
const ZH_CN_CORE: &str = include_str!("../../../locales/zh-CN/core.ftl");
const ZH_CN_READER: &str = include_str!("../../../locales/zh-CN/reader.ftl");
const ZH_CN_CONVERT: &str = include_str!("../../../locales/zh-CN/convert.ftl");
const ZH_CN_SIGN: &str = include_str!("../../../locales/zh-CN/sign.ftl");
const ZH_CN_ERROR: &str = include_str!("../../../locales/zh-CN/error.ftl");
const ZH_CN_EDIT: &str = include_str!("../../../locales/zh-CN/edit.ftl");
const ZH_CN_SEARCH: &str = include_str!("../../../locales/zh-CN/search.ftl");

const EN_US_CORE: &str = include_str!("../../../locales/en-US/core.ftl");
const EN_US_READER: &str = include_str!("../../../locales/en-US/reader.ftl");
const EN_US_CONVERT: &str = include_str!("../../../locales/en-US/convert.ftl");
const EN_US_SIGN: &str = include_str!("../../../locales/en-US/sign.ftl");
const EN_US_ERROR: &str = include_str!("../../../locales/en-US/error.ftl");
const EN_US_EDIT: &str = include_str!("../../../locales/en-US/edit.ftl");
const EN_US_SEARCH: &str = include_str!("../../../locales/en-US/search.ftl");

/// All resources for one locale.
struct LocaleResources {
    #[allow(dead_code)]
    locale: Locale,
    /// A concurrent bundle takes every resource at construction, so custom
    /// catalogues are merged into the source list and the bundle rebuilt.
    bundle: Arc<ConcurrentFluentBundle<FluentResource>>,
    /// Original FTL sources in order, so custom catalogues can be appended.
    sources: Vec<String>,
}

/// Localization context.
/// Thread safe: concurrent bundles behind an `Arc`, so a server shares one
/// instance across requests instead of rebuilding per call.
pub struct I18n {
    current: Locale,
    available: Vec<Locale>,
    resources: HashMap<String, LocaleResources>,
}

impl std::fmt::Debug for I18n {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("I18n")
            .field("current", &self.current)
            .field("available", &self.available)
            .finish()
    }
}

impl I18n {
    /// Start building a context.
    pub fn builder() -> I18nBuilder {
        I18nBuilder::default()
    }

    /// Use the system locale with every built-in catalogue.
    pub fn with_system_locale() -> Self {
        Self::builder().build()
    }

    /// Use an explicit locale.
    pub fn with_locale(locale: Locale) -> Self {
        Self::builder().locale(locale).build()
    }

    /// The locale currently in effect.
    pub fn locale(&self) -> &Locale {
        &self.current
    }

    /// Switch locale, ignoring unknown ones.
    pub fn set_locale(&mut self, locale: Locale) {
        if self.resources.contains_key(locale.tag()) {
            self.current = locale;
        } else {
            tracing::warn!(
                requested = locale.tag(),
                "locale resources not built in, keeping current"
            );
        }
    }

    /// Locales available in this build.
    pub fn available_locales(&self) -> &[Locale] {
        &self.available
    }

    /// Render a message with no arguments.
    pub fn text(&self, id: MessageId) -> String {
        self.text_with(id, None)
    }

    /// Render a message with arguments.
    pub fn text_with<'a, I>(&self, id: MessageId, args: I) -> String
    where
        I: IntoIterator<Item = (&'a str, fluent::FluentValue<'a>)>,
    {
        let key = id.key();
        let mut fa = FluentArgs::new();
        for (k, v) in args {
            fa.set(k, v);
        }

        if let Some(text) = self.lookup(&self.current, key, &fa) {
            return text;
        }

        let fallback = Locale::fallback();
        if self.current != fallback
            && let Some(text) = self.lookup(&fallback, key, &fa)
        {
            tracing::debug!(key, "translation missing, using fallback locale");
            return text;
        }

        tracing::warn!(key, "translation missing with no fallback");
        key.to_string()
    }

    /// Format a key in one locale.
    fn lookup(&self, locale: &Locale, key: &str, args: &FluentArgs<'_>) -> Option<String> {
        let res = self.resources.get(locale.tag())?;
        let msg = res.bundle.get_message(key)?;
        let pattern = msg.value()?;

        let mut errors = Vec::new();
        let formatted = res.bundle.format_pattern(pattern, Some(args), &mut errors);

        if !errors.is_empty() {
            tracing::warn!(key, ?errors, "Fluent formatting reported errors");
        }

        Some(formatted.into_owned())
    }

    /// Keys this locale lacks, for CI consistency checks.
    pub fn missing_keys(&self, locale: &Locale) -> Vec<&'static str> {
        let Some(res) = self.resources.get(locale.tag()) else {
            return Vec::new();
        };
        all_keys()
            .iter()
            .filter(|k| res.bundle.get_message(k).is_none())
            .copied()
            .collect()
    }
}

impl I18n {
    /// Number of locales loaded.
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Whether this is an English-only build, used to trim binary size.
    pub fn is_english_only(&self) -> bool {
        self.available.len() == 1 && self.available[0].tag() == "en-US"
    }
}

/// Builder for [`I18n`].
#[derive(Default)]
pub struct I18nBuilder {
    locale: Option<Locale>,
    negotiation: LocaleNegotiation,
    extra: Vec<(Locale, String)>,
    allow_fallback_merge: bool,
}

impl I18nBuilder {
    /// Request a specific locale.
    pub fn locale(mut self, locale: Locale) -> Self {
        self.locale = Some(locale);
        self
    }

    /// Set the negotiation strategy.
    pub fn negotiation(mut self, n: LocaleNegotiation) -> Self {
        self.negotiation = n;
        self
    }

    /// Inject a custom catalogue, which overrides built-in keys.
    pub fn add_resource(mut self, locale: Locale, ftl: impl Into<String>) -> Self {
        self.extra.push((locale, ftl.into()));
        self
    }

    /// Allow fallback keys to be merged into the current locale.
    /// Off by default; Fluent's own fallback suffices and enabling this defeats
    /// `missing_keys` reporting.
    pub fn allow_fallback_merge(mut self, yes: bool) -> Self {
        self.allow_fallback_merge = yes;
        self
    }

    /// Build the context.
    pub fn build(self) -> I18n {
        let mut resources: HashMap<String, LocaleResources> = HashMap::new();

        #[cfg(feature = "locales-en-us")]
        {
            resources.insert(
                "en-US".to_string(),
                build_bundle(
                    Locale::en_us(),
                    &[
                        EN_US_CORE,
                        EN_US_READER,
                        EN_US_CONVERT,
                        EN_US_SIGN,
                        EN_US_ERROR,
                        EN_US_EDIT,
                        EN_US_SEARCH,
                    ],
                ),
            );
        }

        #[cfg(feature = "locales-zh-cn")]
        {
            resources.insert(
                "zh-CN".to_string(),
                build_bundle(
                    Locale::zh_cn(),
                    &[
                        ZH_CN_CORE,
                        ZH_CN_READER,
                        ZH_CN_CONVERT,
                        ZH_CN_SIGN,
                        ZH_CN_ERROR,
                        ZH_CN_EDIT,
                        ZH_CN_SEARCH,
                    ],
                ),
            );
        }

        // Rebuild rather than append: Fluent resolves later definitions first,
        // so custom sources go last and win.
        for (locale, ftl) in &self.extra {
            let mut sources: Vec<String> = resources
                .get(locale.tag())
                .map(|r| r.sources.clone())
                .unwrap_or_default();
            sources.push(ftl.clone());

            let rebuilt = build_bundle_from_sources(locale.clone(), &sources);
            resources.insert(locale.tag().to_string(), rebuilt);
        }

        let available: Vec<Locale> = BUILTIN_LOCALES
            .iter()
            .filter(|t| resources.contains_key(**t))
            .map(|t| Locale::new(t))
            .collect();

        let current = match self.locale {
            Some(l) => {
                if resources.contains_key(l.tag()) {
                    l
                } else {
                    tracing::warn!(
                        requested = l.tag(),
                        "requested locale unavailable, negotiating"
                    );
                    self.negotiation.negotiate(&available)
                }
            }
            None => self.negotiation.negotiate(&available),
        };

        I18n {
            current,
            available,
            resources,
        }
    }
}

/// Build resources for one locale from borrowed sources.
fn build_bundle(locale: Locale, sources: &[&str]) -> LocaleResources {
    let owned: Vec<String> = sources.iter().map(|s| (*s).to_string()).collect();
    build_bundle_from_sources(locale, &owned)
}

/// Build resources from an ordered source list; later entries win.
fn build_bundle_from_sources(locale: Locale, sources: &[String]) -> LocaleResources {
    let langid = locale.tag().parse().unwrap_or_default();
    let mut bundle = ConcurrentFluentBundle::new_concurrent(vec![langid]);
    bundle.set_use_isolating(false);

    for src in sources {
        match FluentResource::try_new(src.clone()) {
            Ok(res) => {
                if let Err(errs) = bundle.add_resource(res) {
                    tracing::error!(locale = locale.tag(), ?errs, "failed to load FTL resource");
                }
            }
            Err((_, errs)) => {
                tracing::error!(locale = locale.tag(), ?errs, "FTL syntax error");
            }
        }
    }

    LocaleResources {
        locale,
        bundle: Arc::new(bundle),
        sources: sources.to_vec(),
    }
}

/// Every message key, in one static list.
/// Mirrors [`MessageId`] one to one; adding a variant without updating this list
/// fails the consistency test.
pub fn all_keys() -> &'static [&'static str] {
    &[
        "app-name",
        "app-ready",
        "app-loading",
        "app-cancelled",
        "common-confirm",
        "common-cancel",
        "common-save",
        "common-close",
        "common-retry",
        "common-delete",
        "reader-open",
        "reader-page-of",
        "reader-zoom",
        "reader-fit-width",
        "reader-fit-page",
        "reader-layout-single",
        "reader-layout-double",
        "reader-layout-continuous",
        "reader-blank-page",
        "reader-no-match",
        "convert-start",
        "convert-progress",
        "convert-done",
        "convert-failed",
        "convert-output-format",
        "convert-page-range",
        "convert-dpi",
        "convert-keep-bookmarks",
        "convert-keep-signatures",
        "sign-valid",
        "sign-invalid",
        "sign-unverified",
        "sign-expired",
        "sign-integrity-ok",
        "sign-tampered",
        "sign-count",
        "sign-signer",
        "sign-signed-at",
        "sign-algorithm",
        "sign-issuer",
        "sign-export-report",
        "edit-confirm-delete-pages",
        "edit-warn-invalidate-signature",
        "edit-undo",
        "edit-redo",
        "search-result-count",
        "search-placeholder",
        "err-not-ofd",
        "err-password-required",
        "err-page-out-of-range",
        "err-package-corrupted",
        "err-font-missing",
        "err-dangling-reference",
        "err-resource-limit",
        "err-io",
        "err-unsupported",
    ]
}
