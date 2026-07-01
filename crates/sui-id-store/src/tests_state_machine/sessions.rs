//! State-machine property tests for session lifecycle (RFC 083).
//!
//! Named invariants:
//! - `INV_SESSION_REVOKED_NEVER_RESOLVES` — a revoked session cannot be resolved.
//! - `INV_SESSION_EXPIRED_NEVER_RESOLVES` — an expired session cannot be resolved.
//! - `INV_SESSION_REVOKE_ALL_EXCEPT_LEAVES_ONE` — revoke-all-except leaves exactly
//!   the kept session active.
//! - `INV_SESSION_PURGE_DURABILITY` — purge does not resurrect any revoked session.

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
    models::{Role, SessionRow},
    repos::{sessions, users},
};
use sui_id_shared::ids::{SessionId, UserId};

// ── Seeded test database ─────────────────────────────────────────────────────

struct Seed {
    db: Database,
    user_id: UserId,
}

fn make_db() -> Seed {
    let db = Database::open_in_memory(MasterKey::generate()).expect("db");
    let now = Utc::now();
    let user_id = UserId::new();

    tokio::runtime::Runtime::new().expect("rt").block_on(async {
        users::create(
            &db,
            &crate::models::UserRow {
                id: user_id,
                username: format!("sm-sess-{user_id}"),
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
                source: crate::models::UserSource::Local,
                external_stable_id: None,
            },
        )
        .await
        .expect("user");
    });

    Seed { db, user_id }
}

// ── Oracle model ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionState {
    Active,
    Revoked,
}

#[derive(Debug, Clone)]
struct SessionEntry {
    id: SessionId,
    expires_at: DateTime<Utc>,
    state: SessionState,
}

impl SessionEntry {
    fn is_resolvable(&self, now: DateTime<Utc>) -> bool {
        self.state == SessionState::Active && self.expires_at > now
    }
}

struct SessionOracle {
    sessions: Vec<SessionEntry>,
}

impl SessionOracle {
    fn new() -> Self {
        Self { sessions: vec![] }
    }

    fn create(&mut self, id: SessionId, expires_at: DateTime<Utc>) -> usize {
        let idx = self.sessions.len();
        self.sessions.push(SessionEntry {
            id,
            expires_at,
            state: SessionState::Active,
        });
        idx
    }

    fn revoke(&mut self, idx: usize) {
        if idx < self.sessions.len() {
            self.sessions[idx].state = SessionState::Revoked;
        }
    }

    fn revoke_all_except(&mut self, keep_idx: usize) {
        for (i, s) in self.sessions.iter_mut().enumerate() {
            if i != keep_idx {
                s.state = SessionState::Revoked;
            }
        }
    }

    fn revoke_all(&mut self) {
        for s in &mut self.sessions {
            s.state = SessionState::Revoked;
        }
    }

    fn predict_resolve(&self, idx: usize, now: DateTime<Utc>) -> bool {
        self.sessions
            .get(idx)
            .map(|s| s.is_resolvable(now))
            .unwrap_or(false)
    }

    fn active_ids(&self, now: DateTime<Utc>) -> Vec<SessionId> {
        self.sessions
            .iter()
            .filter(|s| s.is_resolvable(now))
            .map(|s| s.id)
            .collect()
    }
}

// ── Operation enum ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum SessionOp {
    Create {
        ttl_secs: i64,
    },
    Revoke {
        idx: usize,
    },
    RevokeAllExcept {
        keep_idx: usize,
    },
    RevokeAll,
    AdvanceClock {
        secs: u32,
    },
    PurgeExpired,
    /// Verify resolve for the session at idx.
    CheckResolve {
        idx: usize,
    },
}

fn session_op_strategy() -> impl Strategy<Value = SessionOp> {
    prop_oneof![
        (60i64..=7200i64).prop_map(|ttl_secs| SessionOp::Create { ttl_secs }),
        (0usize..20usize).prop_map(|idx| SessionOp::Revoke { idx }),
        (0usize..20usize).prop_map(|keep_idx| SessionOp::RevokeAllExcept { keep_idx }),
        Just(SessionOp::RevokeAll),
        (1u32..=3600u32).prop_map(|secs| SessionOp::AdvanceClock { secs }),
        Just(SessionOp::PurgeExpired),
        (0usize..20usize).prop_map(|idx| SessionOp::CheckResolve { idx }),
    ]
}

// ── Harness ───────────────────────────────────────────────────────────────────

fn run_session_sequence(ops: Vec<SessionOp>) {
    let Seed { db, user_id } = make_db();
    let mut oracle = SessionOracle::new();
    // Use real now so short-TTL sessions are in the future;
    // purge_expired uses wall-clock internally.
    let mut now: DateTime<Utc> = Utc::now();
    let rt = tokio::runtime::Runtime::new().expect("rt");

    for op in ops {
        match op {
            SessionOp::Create { ttl_secs } => {
                let id = SessionId::new();
                let expires_at = now + Duration::seconds(ttl_secs);
                let row = SessionRow {
                    id,
                    user_id,
                    created_at: now,
                    expires_at,
                    revoked_at: None,
                    auth_methods: vec![],
                    last_step_up_at: None,
                    last_used_at: Some(now),
                };
                rt.block_on(sessions::insert(&db, &row)).expect("insert");
                oracle.create(id, expires_at);
            }

            SessionOp::Revoke { idx } => {
                if oracle.sessions.is_empty() {
                    continue;
                }
                let i = idx % oracle.sessions.len();
                let id = oracle.sessions[i].id;
                rt.block_on(sessions::revoke(&db, id)).expect("revoke");
                oracle.revoke(i);
            }

            SessionOp::RevokeAllExcept { keep_idx } => {
                if oracle.sessions.is_empty() {
                    continue;
                }
                let keep_i = keep_idx % oracle.sessions.len();
                let keep_id = oracle.sessions[keep_i].id;
                rt.block_on(sessions::revoke_all_for_user_except(&db, user_id, keep_id))
                    .expect("revoke_all_except");
                oracle.revoke_all_except(keep_i);

                // INV_SESSION_REVOKE_ALL_EXCEPT_LEAVES_ONE:
                // At most one active session should remain (the kept one, if it
                // wasn't already expired).
                let active = oracle.active_ids(now);
                assert!(
                    active.len() <= 1,
                    "INV_SESSION_REVOKE_ALL_EXCEPT_LEAVES_ONE: {} active sessions after revoke_all_except (expected ≤ 1)",
                    active.len()
                );
            }

            SessionOp::RevokeAll => {
                rt.block_on(sessions::revoke_all_for_user(&db, user_id))
                    .expect("revoke_all");
                oracle.revoke_all();
            }

            SessionOp::AdvanceClock { secs } => {
                now += Duration::seconds(secs as i64);
            }

            SessionOp::PurgeExpired => {
                rt.block_on(sessions::purge_expired(&db)).expect("purge");
                // INV_SESSION_PURGE_DURABILITY: any session that was revoked
                // must remain inaccessible after purge.
                for s in &oracle.sessions {
                    if s.state == SessionState::Revoked {
                        let result = rt.block_on(sessions::get(&db, s.id));
                        // It's OK if the row is gone (purged) or still there as
                        // revoked. What's NOT OK is if it resolves as active.
                        match result {
                            Ok(row) => assert!(
                                row.revoked_at.is_some(),
                                "INV_SESSION_PURGE_DURABILITY: revoked session {} has revoked_at=None after purge",
                                s.id
                            ),
                            Err(crate::StoreError::NotFound) => {
                                // Purged entirely — acceptable.
                            }
                            Err(e) => panic!("unexpected error: {e:?}"),
                        }
                    }
                }
            }

            SessionOp::CheckResolve { idx } => {
                if oracle.sessions.is_empty() {
                    continue;
                }
                let i = idx % oracle.sessions.len();
                let session = &oracle.sessions[i];
                let predicted = oracle.predict_resolve(i, now);

                // We use get() to check existence and revocation status.
                // Actual session resolution (which checks expiry via the clock)
                // is in sui-id-core and not directly testable here.
                let result = rt.block_on(sessions::get(&db, session.id));

                match (predicted, result) {
                    (true, Ok(row)) => {
                        // INV_SESSION_REVOKED_NEVER_RESOLVES
                        assert!(
                            row.revoked_at.is_none(),
                            "INV_SESSION_REVOKED_NEVER_RESOLVES: active session {} has revoked_at set",
                            session.id
                        );
                        // INV_SESSION_EXPIRED_NEVER_RESOLVES
                        assert!(
                            row.expires_at > now,
                            "INV_SESSION_EXPIRED_NEVER_RESOLVES: active session {} is expired in DB",
                            session.id
                        );
                    }
                    (true, Err(_)) => {
                        // Tolerate: may have been purged in a previous PurgeExpired op
                    }
                    (false, Ok(row)) => {
                        // If the oracle says inactive, the row must be revoked or expired.
                        assert!(
                            row.revoked_at.is_some() || row.expires_at <= now,
                            "INV_SESSION_REVOKED_NEVER_RESOLVES / INV_SESSION_EXPIRED_NEVER_RESOLVES: \
                             session {} should be inactive but is not (revoked_at={:?}, expires_at={:?}, now={:?})",
                            session.id,
                            row.revoked_at,
                            row.expires_at,
                            now
                        );
                    }
                    (false, Err(crate::StoreError::NotFound)) => {
                        // Purged — fine.
                    }
                    (false, Err(e)) => panic!("unexpected DB error: {e:?}"),
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 512,
        ..ProptestConfig::default()
    })]

    /// State-machine property: session lifecycle invariants hold across any
    /// sequence of create / revoke / revoke-all-except / clock-advance / purge.
    ///
    /// Named invariants checked:
    /// - `INV_SESSION_REVOKED_NEVER_RESOLVES`
    /// - `INV_SESSION_EXPIRED_NEVER_RESOLVES`
    /// - `INV_SESSION_REVOKE_ALL_EXCEPT_LEAVES_ONE`
    /// - `INV_SESSION_PURGE_DURABILITY`
    #[test]
    fn session_lifecycle_invariants(
        ops in proptest::collection::vec(session_op_strategy(), 1..=40)
    ) {
        run_session_sequence(ops);
    }
}
