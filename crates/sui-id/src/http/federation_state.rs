//! RFC 096-A prerequisite: moved verbatim out of `handlers/federation.rs`
//! (the preparatory split) — the signed state cookie `federated_start`
//! writes and `federated_callback` consumes (P5).

use crate::AppState;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

// ── State cookie ──────────────────────────────────────────────────────────────

pub const STATE_COOKIE: &str = "sui_id_fed_state";
pub const STATE_TTL_SECS: i64 = 600; // 10 minutes (P5)
const HMAC_KEY_SUFFIX: &[u8] = b":federation-state-v1";

/// Contents of the signed state cookie.
#[derive(Serialize, Deserialize)]
pub struct FedState {
    /// Random nonce (also sent to upstream as `nonce` parameter).
    pub nonce: String,
    /// PKCE code verifier (raw, never sent to upstream).
    pub pkce_verifier: String,
    /// Provider slug for the callback to look up the provider.
    pub provider_slug: String,
    /// Unix timestamp at which this state expires (P5).
    pub expires_at: i64,
    /// Optional `next` URL to redirect to after sign-in.
    pub next: Option<String>,
    /// The `state` parameter sent to the upstream — verified in callback (P5 CSRF).
    pub upstream_state: String,
}

/// Seal the state as `{json}.{hmac_hex}`.
pub fn seal_state(app: &AppState, state: &FedState) -> anyhow::Result<String> {
    let json = serde_json::to_string(state)?;
    let mac = hmac_state(app, json.as_bytes());
    Ok(format!("{json}.{mac}"))
}

/// Verify and unseal a state value from the cookie (P5).
pub fn unseal_state(app: &AppState, raw: &str) -> Option<FedState> {
    let dot = raw.rfind('.')?;
    let (json_part, mac_part) = (&raw[..dot], &raw[dot + 1..]);
    let expected = hmac_state(app, json_part.as_bytes());
    // Constant-time comparison
    use subtle::ConstantTimeEq;
    let ok: bool = expected.as_bytes().ct_eq(mac_part.as_bytes()).into();
    if !ok {
        return None;
    }
    let state: FedState = serde_json::from_str(json_part).ok()?;
    let now = chrono::Utc::now().timestamp();
    if now > state.expires_at {
        return None; // expired
    }
    Some(state)
}

fn hmac_state(app: &AppState, data: &[u8]) -> String {
    let raw_key = app.db.key();
    // Derive a per-use subkey by mixing the master key with a purpose suffix.
    let mut key_material = Vec::with_capacity(32 + HMAC_KEY_SUFFIX.len());
    key_material.extend_from_slice(raw_key.as_bytes());
    key_material.extend_from_slice(HMAC_KEY_SUFFIX);
    // HMAC-SHA256 has no fixed key-size requirement (RFC 2104); new_from_slice
    // only fails for algorithms that do, so this cannot fail here.
    #[allow(clippy::expect_used)]
    let mut mac =
        Hmac::<Sha256>::new_from_slice(&key_material).expect("HMAC accepts any key length");
    mac.update(data);
    let result = mac.finalize().into_bytes();
    result.iter().map(|b| format!("{b:02x}")).collect()
}
