//! Per-locale translation and formatter constants.
//!
//! Each submodule is the single file a translator edits:
//! it contains one `static STRINGS_*` constant (all UI text for that locale)
//! and one `static FORMATTERS_*` constant (date / number formatting).
//!
//! # Subdirectory layout
//!
//! ```text
//! locale/
//!   en.rs        — English (en)
//!   ja.rs        — Japanese (ja)
//!   zh_hans.rs   — Chinese Simplified (zh-Hans)
//!   zh_hant.rs   — Chinese Traditional (zh-Hant) — stub, delegates to zh-Hans
//! ```
//!
//! # Adding a new locale
//!
//! 1. Copy `locale/en.rs` to `locale/xx.rs` and translate every field.
//! 2. Declare `pub mod xx;` here and re-export its constants below.
//! 3. Add a variant to [`crate::Locale`] and wire `tag`, `native_name`,
//!    `strings`, `formatters`, `parse`, and `direction`.
//! 4. Add the variant to [`crate::Locale::ALL`] when the translation is
//!    complete and reviewed.

pub mod en;
pub mod ja;
pub mod zh_hans;
pub mod zh_hant;

pub use en::{FORMATTERS_EN, STRINGS_EN};
pub use ja::{FORMATTERS_JA, STRINGS_JA};
pub use zh_hans::{FORMATTERS_ZH_HANS, STRINGS_ZH_HANS};
pub use zh_hant::{FORMATTERS_ZH_HANT, STRINGS_ZH_HANT};
