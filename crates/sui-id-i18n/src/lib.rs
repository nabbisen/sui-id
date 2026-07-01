//! sui-id internationalisation.
//!
//! ## Design
//!
//! All user-facing strings are fields on a [`Strings`] struct. Each
//! supported locale has a `static Strings` constant with all fields
//! filled in. Adding a locale means adding a variant to [`Locale`] and a
//! new file under `locale/` — the compiler guarantees every translation
//! is complete via the exhaustive `match` in [`Locale::strings`].
//! Adding a string means adding a field to [`Strings`] — the compiler
//! then errors at every per-locale constant until it is filled in.
//!
//! Strings without variable interpolation are `&'static str`. Strings
//! with interpolation use small format functions that take parameters and
//! return `String`. We deliberately avoid a generic templating layer
//! (Fluent, MessageFormat, etc.) — the patterns are simple and
//! per-locale functions are more readable than templated strings.
//!
//! ## What lives here, what doesn't
//!
//! - **Lives here**: UI labels, button text, flash messages,
//!   page titles, email subjects/bodies — anything a human reads.
//! - **Does not live here**: log messages, audit-event names
//!   (stable identifiers operators query against), error machine codes,
//!   configuration keys.
//!
//! ## Module layout
//!
//! - [`strings`] — the [`Strings`] struct (every translatable field).
//! - [`formatters`] — the [`Formatters`] struct + shared helper functions.
//! - [`locale`] — per-locale submodules; each file is self-contained:
//!   one `STRINGS_*` constant and one `FORMATTERS_*` constant.
//!   - `locale/en.rs` — English
//!   - `locale/ja.rs` — Japanese
//!   - `locale/zh_hans.rs` — Chinese Simplified (zh-Hans)
//!   - `locale/zh_hant.rs` — Chinese Traditional (zh-Hant) — stub, see file
//! - [`tests`] — unit tests.
//!
//! ## Adding a locale
//!
//! See `locale.rs` for step-by-step instructions.

mod formatters;
mod locale;
mod strings;
#[cfg(test)]
mod tests;

pub use crate::formatters::Formatters;
pub use crate::locale::{
    FORMATTERS_EN, FORMATTERS_JA, FORMATTERS_ZH_HANS, FORMATTERS_ZH_HANT,
    STRINGS_EN, STRINGS_JA, STRINGS_ZH_HANS, STRINGS_ZH_HANT,
};
pub use crate::strings::Strings;

use serde::{Deserialize, Serialize};

/// A supported locale.
///
/// New variants must:
///   - have a stable BCP-47-style tag returned by [`Locale::tag`];
///   - have a `static STRINGS_*` constant matched in [`Locale::strings`];
///   - have a `static FORMATTERS_*` constant matched in [`Locale::formatters`].
///
/// Add the variant to [`Locale::ALL`] only when the translation is
/// complete and has been reviewed by a native speaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Locale {
    #[serde(rename = "ja")]
    Ja,
    #[serde(rename = "en")]
    En,
    /// Chinese Simplified (zh-Hans / zh-CN).
    ///
    /// Serde accepts the legacy tag `"zh"` as an alias for backward
    /// compatibility with stored preferences written before this split.
    #[serde(rename = "zh-Hans", alias = "zh")]
    ZhHans,
    /// Chinese Traditional (zh-Hant / zh-TW).
    ///
    /// **Not yet in [`Locale::ALL`]** — translations are a stub.
    /// See `locale/zh_hant.rs` for contribution instructions.
    #[serde(rename = "zh-Hant")]
    ZhHant,
}

impl Locale {
    /// All locales available as selectable options in the UI and as
    /// server-default choices. Add a variant here only when its translation
    /// file is complete and reviewed.
    pub const ALL: &'static [Locale] = &[Locale::Ja, Locale::En];

    /// BCP-47 language tag. Used in HTML `lang=` attributes, cookies, and
    /// the user-preference column. Stable — never change without a migration.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Ja     => "ja",
            Self::En     => "en",
            Self::ZhHans => "zh-Hans",
            Self::ZhHant => "zh-Hant",
        }
    }

    /// Native-language name of this locale. Always shown in the locale's
    /// own script so a user who has accidentally landed on the wrong
    /// language can still recognise their own.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::Ja     => "日本語",
            Self::En     => "English",
            Self::ZhHans => "中文（简体）",
            Self::ZhHant => "中文（繁體）",
        }
    }

    /// Parse a BCP-47 tag back into a `Locale`. Tolerant of region and
    /// script suffixes (`en-US` → `En`, `zh-CN` → `ZhHans`,
    /// `zh-TW` → `ZhHant`). Case-insensitive. The bare tag `"zh"` maps
    /// to `ZhHans` (most common web convention and backward-compatible
    /// with preferences stored before the Simplified/Traditional split).
    /// Unknown tags return `None`.
    pub fn parse(tag: &str) -> Option<Locale> {
        // Split on '-' or '_'; match on primary + optional first subtag.
        let mut parts = tag.split(|c: char| c == '-' || c == '_');
        let primary = parts.next().unwrap_or("").to_ascii_lowercase();
        let subtag  = parts.next().map(|s| s.to_ascii_lowercase());
        match (primary.as_str(), subtag.as_deref()) {
            ("ja", _)                              => Some(Locale::Ja),
            ("en", _)                              => Some(Locale::En),
            ("zh", None | Some("hans") | Some("cn") | Some("sg")) => Some(Locale::ZhHans),
            ("zh", Some("hant") | Some("tw") | Some("hk") | Some("mo")) => Some(Locale::ZhHant),
            ("zh", _)                              => Some(Locale::ZhHans), // unknown zh-* → simplified
            _                                      => None,
        }
    }

    /// Strings table for this locale.
    pub fn strings(self) -> &'static Strings {
        match self {
            Self::Ja     => &STRINGS_JA,
            Self::En     => &STRINGS_EN,
            Self::ZhHans => &STRINGS_ZH_HANS,
            Self::ZhHant => &STRINGS_ZH_HANT,
        }
    }

    /// Locale-aware date and number formatters.
    pub fn formatters(self) -> &'static Formatters {
        match self {
            Self::Ja     => &FORMATTERS_JA,
            Self::En     => &FORMATTERS_EN,
            Self::ZhHans => &FORMATTERS_ZH_HANS,
            Self::ZhHant => &FORMATTERS_ZH_HANT,
        }
    }

    /// Text direction. All current locales are LTR; RTL locales will
    /// override this when added (RFC 002 § E).
    pub fn direction(self) -> &'static str {
        match self {
            Self::Ja | Self::En | Self::ZhHans | Self::ZhHant => "ltr",
        }
    }
}

impl Default for Locale {
    fn default() -> Self {
        Locale::Ja
    }
}

/// Pick a locale from a `q`-weighted Accept-Language header.
///
/// Cheap parser: split on commas, take each token's primary subtag (plus
/// the first script/region suffix when present), return the first
/// recognised locale. `q=` weights are ignored — for a small catalogue
/// the cost of a full parser outweighs the benefit.
pub fn negotiate_from_accept_language(header: &str) -> Option<Locale> {
    for raw in header.split(',') {
        let tag = raw.split(';').next().unwrap_or("").trim();
        if let Some(loc) = Locale::parse(tag) {
            return Some(loc);
        }
    }
    None
}
