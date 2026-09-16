//! Pure LDAP filter and DN helpers for authenticating a returning directory
//! user by stable id (roadmap `ldap-returning-signin`, rulings 2 and 3).
//!
//! Kept out of the `ldap` feature gate, so the rules are tested in every
//! lane even though the search and bind that use them are compile-checked
//! only.

/// Escape a value for safe inclusion in an LDAP search filter (RFC 4515 §3).
///
/// The following characters are escaped with a leading backslash and their
/// two-digit hex representation: `* ( ) \ NUL`.  All other bytes pass
/// through unchanged.
pub fn escape_filter_value(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'*' => out.push_str("\\2a"),
            b'(' => out.push_str("\\28"),
            b')' => out.push_str("\\29"),
            b'\\' => out.push_str("\\5c"),
            b'\0' => out.push_str("\\00"),
            other => out.push(other as char),
        }
    }
    out
}

/// `user_search_filter` with every `{username}` replaced by `*`, wrapped in
/// parentheses if the template has none. The directory's own restrictions
/// in the template (a group, an objectClass, an account-status clause)
/// still apply to a search that does not start from a username.
pub fn any_user_filter(user_search_filter: &str) -> String {
    let filter = user_search_filter.replace("{username}", "*");
    let trimmed = filter.trim();
    if trimmed.starts_with('(') {
        trimmed.to_owned()
    } else {
        format!("({trimmed})")
    }
}

/// The search for a stored stable id (ruling 2):
/// `(&(<stable_id_attribute>=<escaped id>)<user_search_filter with * >)`.
pub fn stable_id_filter(
    stable_id_attribute: &str,
    stable_id: &str,
    user_search_filter: &str,
) -> String {
    format!(
        "(&({}={}){})",
        stable_id_attribute,
        escape_filter_value(stable_id),
        any_user_filter(user_search_filter)
    )
}

/// Split a DN into normalised RDNs: split at unescaped `,` or `;`, trim
/// the spaces around each attribute type and value, and lower-case both
/// (directory attribute types and the usual naming values compare
/// case-insensitively). `None` if an RDN has no `=`, so a malformed DN is
/// never treated as lying under the base.
pub fn normalized_rdns(dn: &str) -> Option<Vec<String>> {
    let mut rdns = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for c in dn.chars() {
        if escaped {
            current.push(c);
            escaped = false;
        } else if c == '\\' {
            current.push(c);
            escaped = true;
        } else if c == ',' || c == ';' {
            rdns.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    rdns.push(current);
    if dn.trim().is_empty() {
        return Some(Vec::new());
    }
    rdns.iter()
        .map(|rdn| {
            let (kind, value) = rdn.split_once('=')?;
            let (kind, value) = (kind.trim(), value.trim());
            if kind.is_empty() {
                return None;
            }
            Some(format!("{}={}", kind.to_lowercase(), value.to_lowercase()))
        })
        .collect()
}

/// Whether two DNs name the same entry after normalisation.
pub fn same_dn(a: &str, b: &str) -> bool {
    matches!((normalized_rdns(a), normalized_rdns(b)), (Some(x), Some(y)) if x == y)
}

/// Whether `dn` names an entry strictly under `base` (ruling 3). Both are
/// normalised; a malformed DN is never under anything.
pub fn dn_is_under_base(dn: &str, base: &str) -> bool {
    let (Some(dn), Some(base)) = (normalized_rdns(dn), normalized_rdns(base)) else {
        return false;
    };
    dn.len() > base.len() && dn.ends_with(&base)
}

#[cfg(test)]
mod tests;
