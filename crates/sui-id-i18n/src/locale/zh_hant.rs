//! Chinese Traditional (`zh-Hant`) translation table and date/number formatters.
//!
//! # ⚠️ WORK IN PROGRESS — not yet publicly available
//!
//! This file is a **placeholder stub** that delegates to the Simplified
//! Chinese (`zh-Hans`) constants. Every string and formatter here is
//! Simplified Chinese content and therefore **incorrect** for Traditional
//! Chinese audiences. The stub exists so the type system is satisfied
//! and the `Locale::ZhHant` variant can be compiled and tested without
//! requiring a complete translation upfront.
//!
//! `Locale::ZhHant` is excluded from [`crate::Locale::ALL`] until this
//! file has been reviewed and approved by a Traditional Chinese speaker.
//!
//! # How to contribute translations
//!
//! 1. Copy `locale/zh_hans.rs` to `locale/zh_hant.rs` (replacing this file).
//! 2. Rename `STRINGS_ZH_HANS` → `STRINGS_ZH_HANT` and
//!    `FORMATTERS_ZH_HANS` → `FORMATTERS_ZH_HANT`.
//! 3. Translate all string values to Traditional Chinese (繁體中文).
//!    - Use Traditional script characters (繁體字 vs 简体字).
//!    - Use Taiwan / Hong Kong conventions where they differ from Mainland.
//!    - Adjust the `zh_hant_fmt_relative` function for Traditional phrasing
//!      if needed (e.g. "分鐘前" rather than "分钟前").
//! 4. Add `Locale::ZhHant` to [`crate::Locale::ALL`].
//! 5. Add `ZhHant` radio button to the language picker in
//!    `sui-id-web/src/pages/me_security/language.rs`.
//! 6. Run `cargo test -p sui-id-i18n` to verify completeness.

// Delegate to Simplified Chinese until real translations are contributed.
pub use crate::locale::zh_hans::FORMATTERS_ZH_HANS as FORMATTERS_ZH_HANT;
pub use crate::locale::zh_hans::STRINGS_ZH_HANS as STRINGS_ZH_HANT;
