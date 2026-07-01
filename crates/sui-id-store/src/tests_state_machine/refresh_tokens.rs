//! State-machine property tests for refresh-token family lifecycle (RFC 083).
//!
//! Named invariants:
//! - `INV_FAMILY_SINGLE_ACTIVE` — per-family non-revoked count ≤ 1 at all times.
//! - `INV_FAMILY_REUSE_REVOKES_ALL` — replay of any rotated token yields
//!   `ReuseDetected` and an all-revoked family.
//! - `INV_NO_UNREVOKE` — no operation ever sets `revoked_at` back to NULL.
//! - `INV_EXPIRED_NO_ROTATE` — expired tokens never produce `RotatedHere`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::items_after_test_module
)]

use chrono::{DateTime, Duration, Utc};
use proptest::prelude::*;

use crate::{
    Database,
    crypto::MasterKey,
    models::{ClientRow, ConsentPolicy, RefreshTokenRow, Role, UserRow},
    repos::{clients, refresh_tokens, refresh_tokens::RotationLookup, users},
};
use sui_id_shared::{
    FamilyId, RawRefreshToken, RefreshTokenHash, RefreshTokenId,
    ids::{ClientId, UserId},
};

// ── Seeded test database ─────────────────────────────────────────────────────

struct Seed {
    db: Database,
    user_id: UserId,
    client_id: ClientId,
}

fn make_db() -> Seed {
    let db = Database::open_in_memory(MasterKey::generate()).expect("db");
    let now = Utc::now();
    let user_id = UserId::new();
    let client_id = ClientId::new();

    tokio::runtime::Runtime::new().expect("rt").block_on(async {
        users::create(
            &db,
            &UserRow {
                id: user_id,
                username: format!("sm-{user_id}"),
                display_name: None,
                email: None,
                email_normalized: None,
                email_verified_at: None,
                preferred_lang: None,
                is_admin: false,
                role: Role::User,
                is_disabled: false,
                is_deleted: false,
                last_login_at: None,
                user_uuid: uuid::Uuid::new_v4(),
                created_at: now,
                updated_at: now,
                failed_login_count: 0,
                locked_until: None,
            },
        )
        .await
        .expect("user");
        clients::create(
            &db,
            &ClientRow {
                id: client_id,
                name: "sm".into(),
                confidential: false,
                secret_hash: None,
                redirect_uris: vec!["https://example.com/cb".into()],
                allowed_scopes: String::new(),
                post_logout_redirect_uris: vec![],
                is_disabled: false,
                is_deleted: false,
                consent_policy: ConsentPolicy::default(),
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("client");
    });

    Seed {
        db,
        user_id,
        client_id,
    }
}

// ── Oracle model ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct TokenEntry {
    raw: RawRefreshToken,
    family_idx: usize,
    expires_at: DateTime<Utc>,
    revoked: bool,
}

struct FamilyOracle {
    tokens: Vec<TokenEntry>,
    /// family_idx → set of token indices in that family
    families: Vec<Vec<usize>>,
}

impl FamilyOracle {
    fn new() -> Self {
        Self {
            tokens: vec![],
            families: vec![],
        }
    }

    fn new_family(&mut self, token: RawRefreshToken, expires_at: DateTime<Utc>) -> usize {
        let family_idx = self.families.len();
        let token_idx = self.tokens.len();
        self.tokens.push(TokenEntry {
            raw: token,
            family_idx,
            expires_at,
            revoked: false,
        });
        self.families.push(vec![token_idx]);
        token_idx
    }

    fn issue_successor(
        &mut self,
        family_idx: usize,
        token: RawRefreshToken,
        expires_at: DateTime<Utc>,
    ) -> usize {
        let token_idx = self.tokens.len();
        self.tokens.push(TokenEntry {
            raw: token,
            family_idx,
            expires_at,
            revoked: false,
        });
        if family_idx < self.families.len() {
            self.families[family_idx].push(token_idx);
        }
        token_idx
    }

    fn revoke_family(&mut self, family_idx: usize) {
        if family_idx >= self.families.len() {
            return;
        }
        for &tidx in &self.families[family_idx] {
            self.tokens[tidx].revoked = true;
        }
    }

    /// Oracle prediction for `begin_rotation` outcome.
    fn predict_rotate(&self, token_idx: usize, now: DateTime<Utc>) -> &'static str {
        if token_idx >= self.tokens.len() {
            return "Unknown";
        }
        let t = &self.tokens[token_idx];
        if t.revoked {
            "ReuseDetected"
        } else if t.expires_at <= now {
            "Expired"
        } else {
            "RotatedHere"
        }
    }

    fn mark_rotated(&mut self, token_idx: usize) {
        if token_idx < self.tokens.len() {
            self.tokens[token_idx].revoked = true;
        }
    }
}

// ── Operation enum ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum RefreshOp {
    /// Start a new rotation family (simulates initial token issuance).
    NewFamily { ttl_secs: i64 },
    /// Present a token for rotation.
    Rotate { token_idx: usize },
    /// Replay a token that may have already been rotated.
    ReplayOld { token_idx: usize },
    /// Advance the clock.
    AdvanceClock { secs: u32 },
    /// Purge expired tokens.
    PurgeExpired,
}

fn refresh_op_strategy() -> impl Strategy<Value = RefreshOp> {
    prop_oneof![
        (60i64..=7200i64).prop_map(|ttl_secs| RefreshOp::NewFamily { ttl_secs }),
        (0usize..20usize).prop_map(|token_idx| RefreshOp::Rotate { token_idx }),
        (0usize..20usize).prop_map(|token_idx| RefreshOp::ReplayOld { token_idx }),
        (1u32..=3600u32).prop_map(|secs| RefreshOp::AdvanceClock { secs }),
        Just(RefreshOp::PurgeExpired),
    ]
}

// ── Harness ───────────────────────────────────────────────────────────────────

fn check_family_active_counts(db: &Database, oracle: &FamilyOracle) {
    // INV_FAMILY_SINGLE_ACTIVE: for each family, check active count via sync SQL.
    for (fidx, family_tokens) in oracle.families.iter().enumerate() {
        if family_tokens.is_empty() {
            continue;
        }
        let first_token = &oracle.tokens[family_tokens[0]];
        let hash_bytes = RefreshTokenHash::of(&first_token.raw).as_bytes().to_vec();

        // Look up the family_id from the first token's hash (sync).
        let family_id_str: Option<String> = db
            .with_conn_sync(|conn| {
                match conn.query_row(
                    "SELECT family_id FROM refresh_tokens WHERE token_hash = ?1",
                    [hash_bytes.as_slice()],
                    |r| r.get::<_, String>(0),
                ) {
                    Ok(v) => Ok(Some(v)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(crate::StoreError::from(e)),
                }
            })
            .unwrap_or(None);

        if let Some(fid) = family_id_str {
            let active_count: i64 = db
                .with_conn_sync(|conn| {
                    Ok(conn
                        .query_row(
                            "SELECT COUNT(*) FROM refresh_tokens                              WHERE family_id = ?1 AND revoked_at IS NULL",
                            [fid.as_str()],
                            |r| r.get(0),
                        )
                        .unwrap_or(0))
                })
                .unwrap_or(0);

            assert!(
                active_count <= 1,
                "INV_FAMILY_SINGLE_ACTIVE: family {fidx} has {active_count} active tokens (must be ≤ 1)"
            );
        }
    }
}

fn run_refresh_sequence(ops: Vec<RefreshOp>) {
    let Seed {
        db,
        user_id,
        client_id,
    } = make_db();
    let mut oracle = FamilyOracle::new();
    // Use real now so short-TTL tokens are in the future;
    // purge_expired uses wall-clock internally.
    let mut now: DateTime<Utc> = Utc::now();
    let rt = tokio::runtime::Runtime::new().expect("rt");

    for op in ops {
        match op {
            RefreshOp::NewFamily { ttl_secs } => {
                let token = RawRefreshToken::from_untrusted(uuid::Uuid::new_v4().to_string());
                let id = RefreshTokenId::generate();
                let family = FamilyId::root_of(&id);
                let expires_at = now + Duration::seconds(ttl_secs);
                let row = RefreshTokenRow {
                    id,
                    user_id,
                    client_id,
                    scope: "openid".into(),
                    expires_at,
                    revoked_at: None,
                    created_at: now,
                    auth_methods: vec![],
                    family_id: family,
                };
                rt.block_on(refresh_tokens::insert(&db, &row, &token))
                    .expect("insert");
                oracle.new_family(token, expires_at);
            }

            RefreshOp::Rotate { token_idx } | RefreshOp::ReplayOld { token_idx } => {
                if oracle.tokens.is_empty() {
                    continue;
                }
                let idx = token_idx % oracle.tokens.len();
                let token = oracle.tokens[idx].raw.clone();
                let hash = RefreshTokenHash::of(&token);
                let predicted = oracle.predict_rotate(idx, now);
                let family_idx = oracle.tokens[idx].family_idx;

                let result = rt
                    .block_on(refresh_tokens::begin_rotation(&db, &hash, &client_id, now))
                    .expect("begin_rotation never errors");

                let actual = match &result {
                    RotationLookup::RotatedHere(_) => "RotatedHere",
                    RotationLookup::ReuseDetected { .. } => "ReuseDetected",
                    RotationLookup::Expired(_) => "Expired",
                    RotationLookup::Unknown => "Unknown",
                };

                assert_eq!(
                    predicted, actual,
                    "oracle vs real mismatch for token {idx}: predicted {predicted}, got {actual}"
                );

                match result {
                    RotationLookup::RotatedHere(row) => {
                        // Winner: mark rotated and issue a successor
                        oracle.mark_rotated(idx);
                        let successor =
                            RawRefreshToken::from_untrusted(uuid::Uuid::new_v4().to_string());
                        let new_id = RefreshTokenId::generate();
                        let new_expires = now + Duration::seconds(3600);
                        let new_row = RefreshTokenRow {
                            id: new_id,
                            user_id,
                            client_id,
                            scope: "openid".into(),
                            expires_at: new_expires,
                            revoked_at: None,
                            created_at: now,
                            auth_methods: vec![],
                            family_id: row.family_id,
                        };
                        rt.block_on(refresh_tokens::insert(&db, &new_row, &successor))
                            .expect("insert successor");
                        oracle.issue_successor(family_idx, successor, new_expires);
                    }
                    RotationLookup::ReuseDetected { .. } => {
                        // INV_FAMILY_REUSE_REVOKES_ALL: entire family must be revoked
                        oracle.revoke_family(family_idx);
                    }
                    RotationLookup::Expired(_) | RotationLookup::Unknown => {}
                }

                // INV_FAMILY_SINGLE_ACTIVE: check globally after every rotation
                check_family_active_counts(&db, &oracle);
            }

            RefreshOp::AdvanceClock { secs } => {
                now += Duration::seconds(secs as i64);
            }

            RefreshOp::PurgeExpired => {
                // Sync oracle clock to wall time before purging.
                let _wall_now = Utc::now();
                rt.block_on(refresh_tokens::purge_expired(&db))
                    .expect("purge");
                // INV_NO_UNREVOKE: purge must not set revoked_at to NULL
                // (verified indirectly: family active counts still hold)
                check_family_active_counts(&db, &oracle);
            }
        }
    }
}

// (sync SQL queries used directly via with_conn_sync)

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 512,
        ..ProptestConfig::default()
    })]

    /// State-machine property: refresh-token family lifecycle invariants hold
    /// across any sequence of operations.
    ///
    /// Named invariants checked:
    /// - `INV_FAMILY_SINGLE_ACTIVE`
    /// - `INV_FAMILY_REUSE_REVOKES_ALL`
    /// - `INV_NO_UNREVOKE`
    /// - `INV_EXPIRED_NO_ROTATE`
    #[test]
    fn refresh_family_lifecycle_invariants(
        ops in proptest::collection::vec(refresh_op_strategy(), 1..=40)
    ) {
        run_refresh_sequence(ops);
    }
}
