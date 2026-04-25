//! # sui-id-web
//!
//! Server-rendered HTML for the setup wizard, login screen, and admin
//! dashboard. Each page is produced by a Leptos component rendered with
//! `leptos::prelude::ssr::render_to_string`, which yields a complete HTML
//! string the binary serves directly. No client-side JavaScript or WASM is
//! shipped — that decision keeps the runtime artefact a single static
//! binary, in line with the project's minimalism.
//!
//! Pages do progressive enhancement only: ordinary `<form>` POSTs go back to
//! Axum handlers, which redirect on success.

#![forbid(unsafe_code)]

pub mod layout;
pub mod pages;

pub use pages::{
    render_audit, render_clients, render_dashboard, render_error, render_login, render_setup,
    render_users, Flash, FlashKind,
};
