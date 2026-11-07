//! A `?next=`/`?return_to=`-shaped query value is never a redirect
//! destination on its own say-so: it must first be a same-site, plain
//! path. This module holds only that format check, shared by every site
//! that redirects a signed-in user back to where they came from.
//!
//! `step_up.rs`'s `sanitise_return_to` already enforced these rules
//! (RFC 089) before this module existed; it now calls this function for
//! its format half and keeps its own route allowlist after. The
//! allowlist is a second, independent decision -- which destinations a
//! step-up challenge may return to -- that this module does not make.
//! Callers outside `step_up` choose their own default when this
//! function returns `None` (`/admin`, `/me/security`), rather than
//! sharing one.

/// A same-site, plain path, or `None`.
///
/// Accepts only a value that is:
/// 1. non-empty;
/// 2. starting with a single `/` (not `//`, a protocol-relative URL that
///    leaves the site, and browsers read `/\` the same way);
/// 3. free of `\` anywhere, not only right after the leading `/`;
/// 4. free of any [`char::is_control`] character anywhere (CR, LF, NUL
///    and others) -- broader than the three characters the pre-existing
///    `sanitise_return_to` named individually, a strict widening: every
///    input that function rejected, this still rejects.
///    The tab is the one that matters most: a URL parser deletes every
///    ASCII tab and newline before parsing, and a header value may carry
///    a tab, so `/<TAB>/x` would otherwise be followed as `//x`.
///
/// A percent-encoded character, `%5C` included, is not decoded here and
/// so never trips rule 3 or 4 -- it is still a same-origin path to a
/// browser, which does not decode a path segment before following a
/// `Location` header.
pub fn same_site_return_path(raw: &str) -> Option<&str> {
    if raw.is_empty() {
        return None;
    }
    if !raw.starts_with('/') {
        return None;
    }
    if raw.as_bytes().get(1) == Some(&b'/') {
        return None;
    }
    if raw.contains('\\') {
        return None;
    }
    if raw.chars().any(char::is_control) {
        return None;
    }
    Some(raw)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "same_site_return_path/tests.rs"]
mod tests;
