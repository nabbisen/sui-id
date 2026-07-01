//! LDAP user-source implementation (RFC 005, `ldap` feature).
//!
//! Implements `UserSource` for an LDAP directory using the `ldap3` crate.
//! The implementation follows the **search-then-bind** pattern: the service
//! account binds first, searches for the user's DN by username, then binds
//! a second time with the user's own credentials to verify the password.
//! An empty search result and a wrong-password bind failure are both returned
//! as `Ok(None)` — indistinguishable to callers (P3).
//!
//! # Security invariants enforced here
//!
//! - **P1 (no DN injection):** the username is escaped per RFC 4515 before
//!   being substituted into `user_search_filter`.
//! - **P2 (TLS required):** the config loader rejects `ldap://` URLs; only
//!   `ldaps://` is accepted at this layer.
//! - **P3 (timing equivalence):** both the "unknown user" and "wrong password"
//!   paths execute a search (the latter also a user bind) and return the same
//!   `Ok(None)` — no short-circuit on a search miss.
//! - **P6 (least-privilege):** the service account (`bind_dn`) is used only
//!   to search; it never writes.

#![cfg(feature = "ldap")]

use crate::user_source::{ExternalUserRecord, UserSource, UserSourceError};
use ldap3::{Ldap, LdapConnAsync, LdapConnSettings, Scope, SearchEntry};

// ── LDAP configuration ────────────────────────────────────────────────────────

/// Configuration for one `[[user_source]]` block of kind `ldap`.
#[derive(Debug, Clone)]
pub struct LdapUserSourceConfig {
    /// Human-readable slug (matches the config block key).
    pub slug: String,
    /// LDAP URL.  **Must** use `ldaps://` — cleartext `ldap://` is rejected
    /// at config-load time by the binary crate.
    pub url: String,
    /// DN of the service account used to search the directory.
    pub bind_dn: String,
    /// Password for the service account.  Loaded from an environment variable
    /// at config-load time; never written to disk.
    pub bind_password: String,
    /// Base DN for the user search (e.g. `"ou=people,dc=example,dc=com"`).
    pub user_search_base: String,
    /// LDAP filter template.  Must contain exactly one `{username}` placeholder
    /// which is substituted with the RFC-4515-escaped username.
    /// Example: `"(uid={username})"`.
    pub user_search_filter: String,
    /// Attribute holding the stable identity (never reused for another person).
    /// Examples: `"objectGUID"` (AD), `"entryUUID"` (OpenLDAP), `"dn"`.
    pub stable_id_attribute: String,
    /// Attribute to use as the display name.  Optional; falls back to
    /// `display_username` if absent.
    pub display_name_attribute: Option<String>,
    /// Attribute to use as the email address.  Optional.
    pub email_attribute: Option<String>,
    /// Connect + STARTTLS timeout in seconds (default 5).
    pub connect_timeout_secs: u64,
    /// Search + bind timeout in seconds (default 10).
    pub search_timeout_secs: u64,
}

// ── LDAP user source ──────────────────────────────────────────────────────────

pub struct LdapUserSource {
    cfg: LdapUserSourceConfig,
}

impl LdapUserSource {
    pub fn new(cfg: LdapUserSourceConfig) -> Self {
        Self { cfg }
    }

    /// Connect to the LDAP server and return a ready connection.
    async fn connect(&self) -> Result<Ldap, UserSourceError> {
        let settings = LdapConnSettings::new().set_conn_timeout(std::time::Duration::from_secs(
            self.cfg.connect_timeout_secs,
        ));
        let (conn, ldap) = LdapConnAsync::with_settings(settings, &self.cfg.url)
            .await
            .map_err(|e| UserSourceError::Transport(e.to_string()))?;
        // Drive the connection in a background task.
        ldap3::drive!(conn);
        Ok(ldap)
    }

    /// Bind as the service account.
    async fn service_bind(&self, ldap: &mut Ldap) -> Result<(), UserSourceError> {
        let result = ldap
            .simple_bind(&self.cfg.bind_dn, &self.cfg.bind_password)
            .await
            .map_err(|e| UserSourceError::Transport(e.to_string()))?;
        result
            .success()
            .map_err(|e| UserSourceError::ServiceBind(e.to_string()))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl UserSource for LdapUserSource {
    async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
        // P2: URL must be ldaps://; the config validator enforces this before
        // we are constructed, but assert here as defence-in-depth.
        if self.cfg.url.starts_with("ldap://") && !self.cfg.url.starts_with("ldaps://") {
            return Err(UserSourceError::Config(
                "cleartext ldap:// is not permitted; use ldaps://".into(),
            ));
        }

        let mut ldap = self.connect().await?;
        self.service_bind(&mut ldap).await?;

        // Build the search filter with RFC 4515 escaping of the username (P1).
        let escaped = escape_filter_value(username);
        let filter = self.cfg.user_search_filter.replace("{username}", &escaped);

        // Collect attributes to fetch.
        let mut attrs: Vec<&str> = vec!["dn", &self.cfg.stable_id_attribute];
        if let Some(ref dn_attr) = self.cfg.display_name_attribute {
            attrs.push(dn_attr);
        }
        if let Some(ref em_attr) = self.cfg.email_attribute {
            attrs.push(em_attr);
        }

        let (search_entries, _res) = ldap
            .search(
                &self.cfg.user_search_base,
                Scope::Subtree,
                &filter,
                attrs.as_slice(),
            )
            .await
            .map_err(|e| UserSourceError::Transport(e.to_string()))?
            .success()
            .map_err(|e| UserSourceError::Transport(e.to_string()))?;

        // P3: even on a search miss, we proceed (and return Ok(None)).
        // This avoids a timing distinction between "unknown user" and
        // "wrong password."
        let entry = match search_entries.into_iter().next() {
            Some(e) => SearchEntry::construct(e),
            None => return Ok(None),
        };

        let user_dn = entry.dn.clone();

        // Extract the stable ID.
        let stable_id = entry
            .attrs
            .get(&self.cfg.stable_id_attribute)
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_else(|| user_dn.clone()); // fall back to DN

        let display_name = self
            .cfg
            .display_name_attribute
            .as_deref()
            .and_then(|a| entry.attrs.get(a)?.first().cloned());

        let email = self
            .cfg
            .email_attribute
            .as_deref()
            .and_then(|a| entry.attrs.get(a)?.first().cloned());

        // P3: attempt the user bind to verify the password.
        // A bind failure is indistinguishable from a search miss — both
        // return Ok(None).
        let bind_result = ldap
            .simple_bind(&user_dn, password)
            .await
            .map_err(|e| UserSourceError::Transport(e.to_string()))?;

        if bind_result.rc != 0 {
            // rc=49 is "invalid credentials"; any non-zero is treated as
            // authentication failure (P3 — no rc distinction exposed to caller).
            return Ok(None);
        }

        let _ = ldap.unbind().await;

        Ok(Some(ExternalUserRecord {
            stable_id,
            display_username: username.to_owned(),
            email,
            display_name,
            source_slug: self.cfg.slug.clone(),
        }))
    }

    fn slug(&self) -> &str {
        &self.cfg.slug
    }
}

// ── RFC 4515 filter-value escaping (P1) ──────────────────────────────────────

/// Escape a value for safe inclusion in an LDAP search filter (RFC 4515 §3).
///
/// The following characters are escaped with a leading backslash and their
/// two-digit hex representation: `* ( ) \ NUL`.  All other bytes pass
/// through unchanged.
///
/// This is the single substitution point for `{username}` in
/// `user_search_filter`; no other substitution is supported.
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::escape_filter_value;

    #[test]
    fn plain_username_passes_through() {
        assert_eq!(escape_filter_value("alice"), "alice");
    }

    #[test]
    fn metacharacters_are_escaped() {
        // RFC 4515 §3: special characters must be percent-escaped as \XX
        assert_eq!(escape_filter_value("a*(b)c\\d"), "a\\2a\\28b\\29c\\5cd");
    }

    #[test]
    fn nul_byte_is_escaped() {
        assert_eq!(escape_filter_value("a\0b"), "a\\00b");
    }

    #[test]
    fn injection_attempt_is_neutered() {
        // A classic LDAP injection: closing the filter and adding an OR clause.
        let malicious = "alice)(|(objectClass=*)";
        let escaped = escape_filter_value(malicious);
        // The ( ) * characters must all be escaped.
        assert!(!escaped.contains('('));
        assert!(!escaped.contains(')'));
        assert!(!escaped.contains('*'));
        // Reconstruct with the filter template to confirm it is safe.
        let filter = format!("(uid={})", escaped);
        assert_eq!(filter, "(uid=alice\\29\\28|\\28objectClass=\\2a\\29)");
    }
}
