//! RFC 096-A prerequisite: moved verbatim out of `handlers/federation.rs`
//! (the preparatory split) — username derivation and conflict resolution
//! for a newly provisioned federated user (P7).

use crate::id_token::IdTokenClaims;

/// Derive a candidate username from upstream ID token claims (P7).
pub fn derive_username(claims: &IdTokenClaims) -> String {
    // Priority: preferred_username → email local-part → sub (truncated)
    if let Some(ref pu) = claims.preferred_username {
        let clean: String = pu
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .take(32)
            .collect();
        if !clean.is_empty() {
            return clean;
        }
    }
    if let Some(ref email) = claims.email
        && let Some(local) = email.split('@').next()
    {
        let clean: String = local
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .take(32)
            .collect();
        if !clean.is_empty() {
            return clean;
        }
    }
    // Final fallback: first 16 chars of sub
    claims.sub.chars().take(16).collect()
}

/// Conflict-resolve a proposed username (P7): append numeric suffix until free.
pub async fn resolve_shadow_username(db: &sui_id_store::Database, proposed: &str) -> String {
    if sui_id_store::repos::users::find_by_username(db, proposed)
        .await
        .is_err()
    {
        return proposed.to_owned();
    }
    for n in 2u32..=1000 {
        let candidate = format!("{proposed}{n}");
        if sui_id_store::repos::users::find_by_username(db, &candidate)
            .await
            .is_err()
        {
            return candidate;
        }
    }
    format!("{proposed}-{}", sui_id_shared::ids::UserId::new())
}
