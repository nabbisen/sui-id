//! RFC 094 M2b / cycle J stage 10, fix round, item 3: `gc::run_once` now
//! calls `password_reset_tokens::delete_expired` (previously wired to
//! nothing, despite the function's own "periodic cleanup helper" doc
//! comment — a real retention gap, found by the write-site check, not a
//! naming gap). This proves the wiring behaviourally: an expired row is
//! removed, and an unexpired one survives the same GC cycle.

use chrono::Duration;
use sha2::{Digest, Sha256};
use sui_id_shared::ids::PasswordResetTokenId;
use sui_id_store::models::{PasswordResetTokenRow, ResetTokenOrigin};
use sui_id_store::repos::password_reset_tokens;

use super::common::*;

fn row(
    user_id: sui_id_shared::ids::UserId,
    plaintext: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> PasswordResetTokenRow {
    let now = chrono::Utc::now();
    PasswordResetTokenRow {
        id: PasswordResetTokenId::new(),
        user_id,
        token_hash: Sha256::digest(plaintext.as_bytes()).to_vec(),
        issued_at: now,
        expires_at,
        consumed_at: None,
        requester_ip: None,
        issued_via: ResetTokenOrigin::Email,
        issued_by: None,
        revoked_at: None,
    }
}

#[tokio::test]
async fn gc_removes_an_expired_reset_token_and_keeps_an_unexpired_one() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("find user");
    let _ = session;

    let now = chrono::Utc::now();
    let expired = row(user.id, "expired-token", now - Duration::minutes(1));
    let live = row(user.id, "live-token", now + Duration::minutes(30));
    password_reset_tokens::insert(&state.db, &expired)
        .await
        .expect("insert expired");
    password_reset_tokens::insert(&state.db, &live)
        .await
        .expect("insert live");

    sui_id::gc::run_once(&state).await;

    let remaining: Vec<String> = state
        .db
        .with_conn(|c| {
            let mut stmt = c.prepare("SELECT id FROM password_reset_tokens ORDER BY id")?;
            let ids = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ids)
        })
        .await
        .expect("query");

    assert_eq!(
        remaining,
        vec![live.id.to_string()],
        "the expired row must be gone and the live one must survive the same GC cycle"
    );
}
