//! # sui-id-shared
//!
//! Shared types crossing crate boundaries. Kept intentionally small: only DTOs,
//! protocol-level enums, and a public-facing API error type live here. Internal
//! domain logic stays in `sui-id-core`.

#![forbid(unsafe_code)]

pub mod api;
pub mod auth_method;
pub mod errors;
pub mod ids;

pub use auth_method::{acr_from_methods, amr_from_methods, AuthMethod};
pub use errors::{ApiError, ApiErrorCode};
