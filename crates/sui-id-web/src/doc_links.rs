//! RFC 147 §2/§3: links to this project's own documentation, from error
//! messages and empty-state pages.
//!
//! There is no deployed documentation site — the book is built in CI and
//! never published (RFC 147 N1's correction). So every such link names
//! the doc's source on GitHub, pinned to the binary's own release tag,
//! never a newer `main` that may describe behaviour this binary does
//! not have. Built here once, so the base URL is written in one place.

/// A link to `$path` (a string literal, e.g.
/// `"docs/src/reference/configuration.md"`) as it stood at this
/// binary's own release — `env!("CARGO_PKG_VERSION")`, which every
/// workspace crate shares, so it names this binary's tag whichever
/// crate the macro expands in. Expands to a `&'static str` (`concat!`,
/// not `format!`): every existing caller of `empty_state`'s `cta`
/// passes `&'static str`, and this must too, with no signature change
/// and no allocation.
#[macro_export]
macro_rules! github_doc_url {
    ($path:literal) => {
        concat!(
            "https://github.com/nabbisen/sui-id/blob/",
            env!("CARGO_PKG_VERSION"),
            "/",
            $path
        )
    };
}

#[cfg(test)]
#[path = "doc_links/tests.rs"]
mod tests;
