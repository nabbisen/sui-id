//! Page renderers.
//!
//! Each screen lives in its own child module under `pages/`. The big
//! monolithic `pages.rs` (4170 LOC at v0.46.0) is split per RFC 065
//! into per-screen modules so each file stays inside the 500-LOC
//! recommend ceiling.
//!
//! The public surface (`render_*`, `*Data` types, `Flash`,
//! `FlashKind`, `EmptyStateData`, `confirm_screen`, etc.) is
//! re-exported transparently — external callers (handlers crate)
//! see no change.

pub mod common;

pub mod audit;
pub mod auth;
pub mod clients;
pub mod confirm;
pub mod dashboard;
pub mod error;
pub mod me_security;
pub mod oidc;
pub mod settings;
pub mod setup;
pub mod signing_keys;
pub mod users;

// Flat public surface — every external caller (lib.rs re-export +
// handlers) uses `sui_id_web::render_dashboard` etc., not
// `sui_id_web::pages::dashboard::render_dashboard`. `pub use *` from
// each submodule reconstructs the flat interface that lib.rs
// previously had directly.
pub use audit::*;
pub use auth::*;
pub use clients::*;
pub use common::{
    EmptyStateAction, EmptyStateData, Flash, FlashKind, empty_state, table_empty_row,
};
pub use confirm::*;
pub use dashboard::*;
pub use error::*;
pub use me_security::*;
pub use oidc::*;
pub use settings::*;
pub use setup::*;
pub use signing_keys::*;
pub use users::*;
