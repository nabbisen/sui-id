//! Settings sub-screens (RFC 065 sub-split).

use super::common::*;
use leptos::prelude::*;

mod authentication;
/// Two-column key/value table used inside each settings card. Keeps
/// per-tab content boring and consistent.
mod basic;
mod email;
mod logs;
mod other;
mod security;

pub use authentication::*;
pub use basic::*;
pub use email::*;
pub use logs::*;
pub use other::*;
pub use security::*;
