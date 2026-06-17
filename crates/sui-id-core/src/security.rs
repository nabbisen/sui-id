//! Security levels — coherent sets of policy thresholds for different
//! deployment contexts, modelled after browser security/privacy tiers.
//!
//! Each level is a *named, documented default* rather than a collection
//! of ad-hoc constants. Adding a new threshold (e.g. session length,
//! token lifetime) means adding one method here; no scattered constants.
//!
//! # Usage
//!
//! ```rust
//! let min = SecurityLevel::Standard.password_min_len(); // 12
//! let min = SecurityLevel::Development.password_min_len(); // 8
//! ```
//!
//! The active level is derived from `AppState::security_level()` in the
//! binary crate; core functions receive `min_len: usize` (or a similar
//! primitive) so they remain unaware of the run mode.

/// Security level governing minimum-security policy thresholds.
///
/// Production deployments run at [`SecurityLevel::Standard`].
/// Local development uses [`SecurityLevel::Development`] (set by the
/// `--dev` flag) to reduce friction without touching any production
/// code path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLevel {
    /// Full security for production deployments.
    ///
    /// All thresholds follow NIST SP 800-63B and the project's security
    /// design. This is the default when sui-id starts without `--dev`.
    Standard,

    /// Relaxed thresholds for local development.
    ///
    /// Allows short test credentials, disables HIBP, lifts lockout,
    /// and relaxes cookie security. **Never use in production.**
    Development,
}

impl SecurityLevel {
    /// Minimum password character count enforced at this level.
    ///
    /// `Standard` → 12. `Development` → 8, allowing common test
    /// credentials such as `"changeme"` (8 chars) without affecting
    /// any production path.
    ///
    /// NIST SP 800-63B §5.1.1 mandates at least 8; 12 is the
    /// project's production floor.
    pub fn password_min_len(self) -> usize {
        match self {
            Self::Standard    => 12,
            Self::Development => 8,
        }
    }
}
