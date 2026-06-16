//! Component styles — base reset, typography, primitive UI components.
//!
//! ## Sharded structure (RFC-MI-010, v0.50.0)
//!
//! What used to be a single 1094-line `components.rs` is now split
//! into eleven shards under `components/`. Each shard owns one
//! user-facing concern and may grow naturally with the mockup
//! integration work:
//!
//! | Shard             | Concern                                                 |
//! |-------------------|---------------------------------------------------------|
//! | `chrome.rs`       | base reset, typography, Shell layout, page-header, theme toggle, responsive |
//! | `cards.rs`        | card, panel, callout, metric, empty-state primitives     |
//! | `forms.rs`        | label, hint, validation, required marker                |
//! | `tables.rs`       | table, wrapping, copy-cell affordances                  |
//! | `buttons.rs`      | button variants (primary, secondary, danger, ghost, link) |
//! | `banners.rs`      | inline flash, standalone banners, dev-mode banner       |
//! | `badges.rs`       | `status_badge`, `StatusKind`, status CSS variants       |
//! | `tabs.rs`         | route-based tab strips                                  |
//! | `confirm.rs`      | reversibility badge, confirm-shell visual cues          |
//! | `setup.rs`        | auth-card centred layout, setup-wizard language picker  |
//! | `utilities.rs`    | the RFC 067 bounded utility-class set                   |
//!
//! [`COMPONENTS_CSS`] interleaves each shard's sub-constants in the
//! exact source order of the pre-split `components.rs` so cascade
//! semantics are byte-identical to v0.49.x. Verification is
//! straightforward: run `cargo test --workspace` and inspect any
//! representative page (dashboard, login, settings, audit, confirm)
//! for visual identity to the prior release.
//!
//! ## Authoring contract
//!
//! Every value here resolves through a token. Grep for a hex value:
//! you should find none. If you need to tune a colour, edit
//! `tokens.rs`; if you need to tune a shape (radius, padding,
//! shadow), still edit `tokens.rs` since those are tokens too.
//!
//! Components ship with sensible defaults so most pages compose them
//! by class, not by inline style. Page-specific code is allowed to
//! override via more-specific selectors but should never reach for
//! inline `style="..."` for visual concerns.
//!
//! New utility classes require RFC justification (RFC 067).

use std::sync::OnceLock;

pub mod badges;
pub mod banners;
pub mod buttons;
pub mod cards;
pub mod chrome;
pub mod confirm;
pub mod forms;
pub mod setup;
pub mod tables;
pub mod tabs;
pub mod utilities;

// Backward-compatible re-export for existing call sites. RFC-MI-010
// is class-preserving and contract-preserving; `crate::components::
// status_badge` and `crate::components::StatusKind` continue to
// resolve unchanged.
pub use badges::{status_badge, StatusKind};
pub use tabs::{route_tabs, RouteTab};
pub use setup::{SetupStep, StepState};
pub use forms::FieldError;

/// The concatenated component stylesheet served by [`crate::layout`].
///
/// Returns a `&'static str` formed by concatenating each shard's
/// `pub const _CSS: &str` sub-constants in the exact source order
/// of the pre-shard `components.rs` (v0.49.x). The result is
/// byte-equivalent CSS content — every rule appears in the same
/// position in the cascade, ensuring no visual or specificity
/// regression.
///
/// Built once at first call via [`OnceLock`] and cached for the
/// lifetime of the process. Calling once at startup (during the
/// first `render_*` invocation) is the dominant pattern; subsequent
/// calls just return the cached pointer.
///
/// ### Why a function and not a `const`?
///
/// Rust's `concat!()` macro only accepts string literals, not
/// `const` items. A `pub const COMPONENTS_CSS: &str = concat!(...)`
/// over per-shard constants will not compile. The runtime concat is
/// the minimal-impact alternative: it preserves the `&'static str`
/// return type (so call sites need only swap `COMPONENTS_CSS` for
/// `components_css()`) and adds zero runtime cost beyond the first
/// call. The cached value never re-allocates.
pub fn components_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        [
            // ---- chrome part 1: base reset + typography + layout chrome ----
            chrome::CHROME_BASE_CSS,
            chrome::CHROME_TYPOGRAPHY_CSS,
            chrome::CHROME_LAYOUT_CSS,
            // ---- cards ----
            cards::CARDS_CSS,
            // ---- utilities part 1: RFC 067 utility-class block ----
            utilities::UTILITIES_RFC067_CSS,
            // ---- forms ----
            forms::FORMS_CSS,
            // ---- buttons ----
            buttons::BUTTONS_CSS,
            // ---- tables ----
            tables::TABLES_CSS,
            // ---- badges part 1: base + variants ----
            badges::BADGES_BASE_CSS,
            // ---- banners part 1: inline flash + standalone banners ----
            banners::BANNERS_FLASH_CSS,
            banners::BANNERS_STATUS_CSS,
            // ---- chrome part 2: page-header + theme-toggle ----
            chrome::CHROME_PAGE_HEADER_CSS,
            chrome::CHROME_THEME_TOGGLE_CSS,
            // ---- setup part 1: auth-card centred layout ----
            setup::SETUP_AUTH_CARD_CSS,
            // ---- utilities part 2: dividers + visually-hidden + copy button ----
            utilities::UTILITIES_DIVIDERS_CSS,
            utilities::UTILITIES_VISUALLY_HIDDEN_CSS,
            utilities::UTILITIES_COPY_BUTTON_CSS,
            // ---- tabs ----
            tabs::TABS_CSS,
            // ---- banners part 2: dev-mode banner ----
            banners::BANNERS_DEVMODE_CSS,
            // ---- utilities part 3: motion / transition tokens ----
            utilities::UTILITIES_MOTION_CSS,
            // ---- confirm / step-up surfaces ----
            confirm::CONFIRM_CSS,
            // ---- badges part 2: muted variant ----
            badges::BADGES_MUTED_CSS,
            // ---- utilities part 4: additional utility-class block ----
            utilities::UTILITIES_ADDITIONAL_CSS,
            // ---- setup part 2: setup-wizard language picker ----
            setup::SETUP_LANG_PICKER_CSS,
            // ---- chrome part 3: responsive @media breakpoints ----
            chrome::CHROME_RESPONSIVE_CSS,
        ]
        .concat()
    })
}
