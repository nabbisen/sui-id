//! WebAuthn / passkey use cases.
//!
//! Wraps the `webauthn-rs` crate (the safe, high-level wrapper) into
//! sui-id's storage layer.
//!
//! Two ceremonies, mirroring the WebAuthn spec:
//!
//! 1. **Registration.** A logged-in user enrols a new passkey.
//!    `start_registration` calls `webauthn-rs::start_passkey_registration`,
//!    serialises the in-progress state to a `webauthn_pending` row, and
//!    returns the `CreationChallengeResponse` JSON for the browser.
//!    `finish_registration` consumes the row and the browser-supplied
//!    `RegisterPublicKeyCredential`, lets webauthn-rs verify, and
//!    persists the resulting `Passkey` sealed under the master key.
//!
//! 2. **Authentication.** Same pattern, with `start_passkey_authentication`
//!    + `finish_passkey_authentication`. On success the matching
//!    credential's signature counter is updated and a session is
//!    promoted (the bin layer wraps this together with the pending-MFA
//!    cookie flow).

use crate::errors::{CoreError, CoreResult};
use crate::time::SharedClock;
use chrono::Duration;
use sui_id_shared::ids::{UserId, WebauthnCredentialId, WebauthnPendingId};
use sui_id_store::Database;
use sui_id_store::models::{UserWebauthnCredentialRow, WebauthnPendingKind, WebauthnPendingRow};
use sui_id_store::repos::{user_webauthn_credentials, users, webauthn_pending};
use webauthn_rs::prelude::{
    Passkey, PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential,
    RegisterPublicKeyCredential, Webauthn, WebauthnBuilder,
};

/// Build a `Webauthn` instance from a sui-id issuer URL.
///
/// `issuer_url` is the URL the operator advertises in `iss` (taken from
/// the `Config::server.public_url`). The WebAuthn `rp_origin` must
/// match the browser's `window.location.origin` at challenge time —
/// scheme, host, and port. `rp_id` is the bare host.
///
/// # Transport enforcement (RFC 011)
///
/// The WebAuthn spec requires ceremonies to run over HTTPS or on
/// `localhost` HTTP. The browser enforces this on its side; sui-id
/// additionally enforces it here so a misconfigured deployment fails
/// fast at startup with a clear error, matching the project's
/// fail-loud-at-startup posture for other config invariants.
///
/// Accepted combinations:
/// - `https://` with any host
/// - `http://` with `localhost`, `127.0.0.1`, or `::1`
pub async fn build(issuer_url: &str) -> CoreResult<Webauthn> {
    let parsed = url::Url::parse(issuer_url).map_err(|_| CoreError::Internal)?;
    let scheme = parsed.scheme();
    let host = parsed.host_str().ok_or(CoreError::Internal)?;

    // Server-enforced WebAuthn transport invariant.
    // Spec requirement: WebAuthn must run over HTTPS or http on localhost.
    let is_localhost = matches!(host, "localhost" | "127.0.0.1" | "::1");
    if scheme != "https" && !(scheme == "http" && is_localhost) {
        return Err(CoreError::ConfigError(format!(
            "WebAuthn requires https, or http on localhost; \
             got {scheme}://{host} — update server.public_url to an https URL \
             (or use http://localhost for local development)"
        )));
    }

    let rp_id = host.to_owned();
    // Trim trailing slash; webauthn-rs is strict on origin formatting.
    let mut origin_str = format!("{scheme}://{host}");
    if let Some(port) = parsed.port() {
        origin_str.push_str(&format!(":{port}"));
    }
    let origin = url::Url::parse(&origin_str).map_err(|_| CoreError::Internal)?;
    let builder = WebauthnBuilder::new(&rp_id, &origin).map_err(|_| CoreError::Internal)?;
    let builder = builder.rp_name("sui-id");
    builder.build().map_err(|_| CoreError::Internal)
}

const PENDING_TTL_SECS: i64 = 5 * 60;

// ---------- registration ----------

pub struct RegistrationStart {
    /// JSON the browser passes to `navigator.credentials.create()`.
    pub challenge_json: String,
    /// Cookie value the caller hands to the user. Maps to the
    /// `webauthn_pending` row.
    pub pending_id: WebauthnPendingId,
}

pub async fn start_registration(
    db: &Database,
    clock: &SharedClock,
    issuer_url: &str,
    user_id: UserId,
) -> CoreResult<RegistrationStart> {
    let webauthn = build(issuer_url).await?;
    let user = users::get(db, user_id).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    let display = user
        .display_name
        .clone()
        .unwrap_or_else(|| user.username.clone());
    // Exclude credentials this user has already registered, so a
    // second-attempt scan from the same authenticator gets a useful
    // error rather than a duplicate.
    let exclude: Vec<webauthn_rs::prelude::CredentialID> =
        user_webauthn_credentials::list_for_user(db, user_id)
            .await?
            .into_iter()
            .map(|c| webauthn_rs::prelude::CredentialID::from(c.credential_id))
            .collect();
    let exclude = if exclude.is_empty() {
        None
    } else {
        Some(exclude)
    };
    let (ccr, reg_state) = webauthn
        .start_passkey_registration(user.user_uuid, &user.username, &display, exclude)
        .map_err(|_| CoreError::Internal)?;
    let state_json = serde_json::to_string(&reg_state).map_err(|_| CoreError::Internal)?;
    let now = clock.now();
    let pending = WebauthnPendingRow {
        id: WebauthnPendingId::new(),
        kind: WebauthnPendingKind::Register,
        user_id: Some(user_id),
        state_json,
        expires_at: now + Duration::seconds(PENDING_TTL_SECS),
        created_at: now,
    };
    webauthn_pending::insert(db, &pending).await?;
    let challenge_json = serde_json::to_string(&ccr).map_err(|_| CoreError::Internal)?;
    Ok(RegistrationStart {
        challenge_json,
        pending_id: pending.id,
    })
}

pub async fn finish_registration(
    db: &Database,
    clock: &SharedClock,
    issuer_url: &str,
    pending_id: WebauthnPendingId,
    user_id: UserId,
    nickname: &str,
    credential: &RegisterPublicKeyCredential,
) -> CoreResult<UserWebauthnCredentialRow> {
    let webauthn = build(issuer_url).await?;
    let pending = webauthn_pending::get(db, pending_id)
        .await?
        .ok_or(CoreError::Unauthenticated)?;
    if pending.expires_at < clock.now()
        || pending.kind != WebauthnPendingKind::Register
        || pending.user_id != Some(user_id)
    {
        let _ = webauthn_pending::delete(db, pending_id).await;
        return Err(CoreError::Unauthenticated);
    }
    let reg_state: PasskeyRegistration =
        serde_json::from_str(&pending.state_json).map_err(|_| CoreError::Internal)?;
    let passkey = webauthn
        .finish_passkey_registration(credential, &reg_state)
        .map_err(|_| CoreError::BadRequest("WebAuthn registration verification failed".into()))?;
    let credential_id_bytes: Vec<u8> = passkey.cred_id().as_ref().to_vec();
    let passkey_json = serde_json::to_vec(&passkey).map_err(|_| CoreError::Internal)?;
    let now = clock.now();
    let nickname = if nickname.trim().is_empty() {
        "Passkey".to_string()
    } else {
        nickname.trim().to_string()
    };
    let row = UserWebauthnCredentialRow {
        id: WebauthnCredentialId::new(),
        user_id,
        credential_id: credential_id_bytes,
        passkey_enc: Vec::new(), // filled by repo::create after sealing
        nickname,
        created_at: now,
        last_used_at: None,
    };
    user_webauthn_credentials::create(db, &row, &passkey_json).await?;
    let _ = webauthn_pending::delete(db, pending_id).await;
    Ok(row)
}

// ---------- authentication ----------

pub struct AuthenticationStart {
    pub challenge_json: String,
    pub pending_id: WebauthnPendingId,
}

pub async fn start_authentication(
    db: &Database,
    clock: &SharedClock,
    issuer_url: &str,
    user_id: UserId,
) -> CoreResult<AuthenticationStart> {
    let webauthn = build(issuer_url).await?;
    let creds = user_webauthn_credentials::list_for_user(db, user_id).await?;
    if creds.is_empty() {
        return Err(CoreError::BadRequest(
            "no WebAuthn credentials enrolled for this user".into(),
        ));
    }
    let mut passkeys: Vec<Passkey> = Vec::with_capacity(creds.len());
    for c in &creds {
        let blob = user_webauthn_credentials::decrypt_passkey(db, c).await?;
        let pk: Passkey = serde_json::from_slice(&blob).map_err(|_| CoreError::Internal)?;
        passkeys.push(pk);
    }
    let (rcr, auth_state) = webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|_| CoreError::Internal)?;
    let state_json = serde_json::to_string(&auth_state).map_err(|_| CoreError::Internal)?;
    let now = clock.now();
    let pending = WebauthnPendingRow {
        id: WebauthnPendingId::new(),
        kind: WebauthnPendingKind::Authenticate,
        user_id: Some(user_id),
        state_json,
        expires_at: now + Duration::seconds(PENDING_TTL_SECS),
        created_at: now,
    };
    webauthn_pending::insert(db, &pending).await?;
    let challenge_json = serde_json::to_string(&rcr).map_err(|_| CoreError::Internal)?;
    Ok(AuthenticationStart {
        challenge_json,
        pending_id: pending.id,
    })
}

pub async fn finish_authentication(
    db: &Database,
    clock: &SharedClock,
    issuer_url: &str,
    pending_id: WebauthnPendingId,
    expected_user_id: UserId,
    credential: &PublicKeyCredential,
) -> CoreResult<()> {
    let webauthn = build(issuer_url).await?;
    let pending = webauthn_pending::get(db, pending_id)
        .await?
        .ok_or(CoreError::Unauthenticated)?;
    if pending.expires_at < clock.now()
        || pending.kind != WebauthnPendingKind::Authenticate
        || pending.user_id != Some(expected_user_id)
    {
        let _ = webauthn_pending::delete(db, pending_id).await;
        return Err(CoreError::Unauthenticated);
    }
    let auth_state: PasskeyAuthentication =
        serde_json::from_str(&pending.state_json).map_err(|_| CoreError::Internal)?;
    let result = webauthn
        .finish_passkey_authentication(credential, &auth_state)
        .map_err(|_| CoreError::Unauthenticated)?;

    // Update the matching credential's stored passkey blob (for the
    // signature counter, in particular) and bump last_used_at.
    let row = user_webauthn_credentials::find_by_credential_id(db, result.cred_id().as_ref())
        .await?
        .ok_or(CoreError::Unauthenticated)?;
    if row.user_id != expected_user_id {
        // The credential id points at a different user — protocol
        // violation or attempted impersonation. Refuse.
        return Err(CoreError::Unauthenticated);
    }
    let mut passkey: Passkey = {
        let blob = user_webauthn_credentials::decrypt_passkey(db, &row).await?;
        serde_json::from_slice(&blob).map_err(|_| CoreError::Internal)?
    };
    let _changed = passkey.update_credential(&result);
    let new_blob = serde_json::to_vec(&passkey).map_err(|_| CoreError::Internal)?;
    user_webauthn_credentials::update_passkey(db, row.id, &new_blob).await?;
    let _ = webauthn_pending::delete(db, pending_id).await;
    Ok(())
}

// ---------- listing / management ----------

pub struct CredentialDescriptor {
    pub id: WebauthnCredentialId,
    pub nickname: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_for_user(
    db: &Database,
    user_id: UserId,
) -> CoreResult<Vec<CredentialDescriptor>> {
    Ok(user_webauthn_credentials::list_for_user(db, user_id)
        .await?
        .into_iter()
        .map(|r| CredentialDescriptor {
            id: r.id,
            nickname: r.nickname,
            created_at: r.created_at,
            last_used_at: r.last_used_at,
        })
        .collect())
}

pub async fn delete(
    db: &Database,
    user_id: UserId,
    credential_id: WebauthnCredentialId,
) -> CoreResult<()> {
    user_webauthn_credentials::delete(db, credential_id, user_id)
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::NotFound => CoreError::NotFound,
            other => CoreError::from(other),
        })?;
    Ok(())
}

/// True if the user has at least one passkey registered.
pub async fn has_credentials(db: &Database, user_id: UserId) -> CoreResult<bool> {
    Ok(user_webauthn_credentials::count_for_user(db, user_id).await? > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn build_accepts_https_url() {
        let w = build("https://idp.example/").await.expect("build");
        let _ = w; // we just want it to construct without panicking.
    }

    #[tokio::test]
    async fn build_accepts_https_with_port() {
        let w = build("https://idp.example:8443/").await.expect("build");
        let _ = w;
    }

    #[tokio::test]
    async fn build_accepts_localhost_http() {
        let w = build("http://localhost:8080/")
            .await
            .expect("localhost http");
        let _ = w;
    }

    #[tokio::test]
    async fn build_accepts_127_0_0_1_http() {
        // webauthn-rs requires a hostname for rp_id, not a raw IP address.
        // 127.0.0.1 passes our transport check (it's a loopback address)
        // but is rejected at the WebauthnBuilder level with an Err — which
        // is the correct behaviour: operators should use `localhost`, not the
        // numeric address, for local dev.  We test that we reach the builder
        // stage (i.e., our transport guard doesn't reject it) by checking that
        // the error, if any, is *not* a ConfigError.
        let r = build("http://127.0.0.1:8801/").await;
        match r {
            Ok(_) => {} // webauthn-rs accepted it — fine
            Err(CoreError::ConfigError(_)) => {
                panic!("127.0.0.1 http must not be rejected by our transport guard (RFC 011)")
            }
            Err(_) => {} // webauthn-rs rejected the IP as rp_id — expected
        }
    }

    // RFC 011: http on a non-localhost host must be rejected at startup.
    #[tokio::test]
    async fn build_rejects_http_on_public_host() {
        let r = build("http://idp.example/").await;
        assert!(
            r.is_err(),
            "http on a non-localhost host must be rejected (RFC 011)"
        );
        let err = r.unwrap_err();
        assert!(
            matches!(err, CoreError::ConfigError(_)),
            "expected ConfigError, got: {err}"
        );
    }

    #[tokio::test]
    async fn build_rejects_url_without_host() {
        // file:// has no host — webauthn-rs (and our wrapper) reject this.
        let r = build("file:///etc/passwd").await;
        assert!(r.is_err());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::time::system_clock;
    use sui_id_store::Database;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::UserRow;
    use sui_id_store::repos::{users, webauthn_pending};

    async fn fresh_db_with_user() -> (Database, UserId) {
        let key = MasterKey::generate();
        let db = Database::open_in_memory(key).expect("db");
        let uid = UserId::new();
        users::create(
            &db,
            &UserRow {
                id: uid,
                username: "alice".into(),
                display_name: None,
                is_admin: true,
                role: if true {
                    sui_id_store::models::Role::Admin
                } else {
                    sui_id_store::models::Role::User
                },
                last_login_at: None,
                is_disabled: false,
                is_deleted: false,
                user_uuid: uuid::Uuid::new_v4(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                failed_login_count: 0,
                locked_until: None,
                email: None,
                preferred_lang: None,
                email_normalized: None,
                email_verified_at: None,
            },
        )
        .await
        .expect("insert user");
        (db, uid)
    }

    #[tokio::test]
    async fn start_registration_persists_pending_row_and_returns_challenge_json() {
        let (db, uid) = fresh_db_with_user().await;
        let clock = system_clock();
        let started = start_registration(&db, &clock, "https://idp.example", uid)
            .await
            .expect("start");
        // Pending row must exist and be of kind Register.
        let row = webauthn_pending::get(&db, started.pending_id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(
            row.kind,
            sui_id_store::models::WebauthnPendingKind::Register
        );
        assert_eq!(row.user_id, Some(uid));
        // Challenge JSON should parse and contain a publicKey.challenge.
        let v: serde_json::Value = serde_json::from_str(&started.challenge_json).expect("json");
        assert!(v.get("publicKey").is_some(), "got: {v}");
    }

    #[tokio::test]
    async fn start_authentication_rejects_users_with_no_credentials() {
        let (db, uid) = fresh_db_with_user().await;
        let clock = system_clock();
        let r = start_authentication(&db, &clock, "https://idp.example", uid).await;
        assert!(matches!(r, Err(crate::errors::CoreError::BadRequest(_))));
    }

    #[tokio::test]
    async fn finish_registration_rejects_expired_pending_row() {
        // Manufacture a pending row that has already expired and verify
        // finish_registration refuses it (returns Unauthenticated).
        use sui_id_store::models::{WebauthnPendingKind, WebauthnPendingRow};
        let (db, uid) = fresh_db_with_user().await;
        let clock = system_clock();
        let now = clock.now();
        let pending_id = sui_id_shared::ids::WebauthnPendingId::new();
        webauthn_pending::insert(
            &db,
            &WebauthnPendingRow {
                id: pending_id,
                kind: WebauthnPendingKind::Register,
                user_id: Some(uid),
                state_json: "{}".into(),
                expires_at: now - chrono::Duration::seconds(1),
                created_at: now - chrono::Duration::seconds(601),
            },
        )
        .await
        .expect("insert");
        // Build a dummy credential JSON; we never get past the expiry
        // check, so its content does not matter — but it must
        // syntactically deserialise (the `rawId`/binary fields parse as
        // base64url-no-pad).
        let dummy: webauthn_rs::prelude::RegisterPublicKeyCredential = serde_json::from_str(
            r#"{"id":"AA","rawId":"AA","type":"public-key","response":{"attestationObject":"AA","clientDataJSON":"AA"},"extensions":{}}"#,
        )
        .expect("parse dummy");
        let r = finish_registration(
            &db,
            &clock,
            "https://idp.example",
            pending_id,
            uid,
            "test",
            &dummy,
        )
        .await;
        assert!(matches!(r, Err(crate::errors::CoreError::Unauthenticated)));
    }
}
