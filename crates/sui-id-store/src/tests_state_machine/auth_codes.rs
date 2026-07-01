//! State-machine property tests for authorization code lifecycle (RFC 083).
//!
//! An in-memory oracle model tracks which codes have been issued, consumed,
//! and expired. After every generated operation, the real `Database` state
//! must match the oracle's predictions. Sequence lengths 1–40, 256 cases.
//!
//! Named invariants (greppable):
//! - `INV_CODE_SINGLE_USE` — at most one successful consume per code.
//! - `INV_CODE_NO_EXPIRY_CONSUME` — no success after the code's expiry time.
//! - `INV_CODE_PURGE_NO_RESURRECT` — purge never reverts consumed/expired state.

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
    models::{AuthorizationCodeRow, ClientRow, ConsentPolicy, Role, UserRow},
    repos::{auth_codes, clients, users},
};

use sui_id_shared::{
    CodeHash,
    ids::{ClientId, UserId},
};

// ── Seeded test database ─────────────────────────────────────────────────────

struct SeededDb {
    db: Database,
    user_id: UserId,
    client_id: ClientId,
}

fn make_seeded_db() -> SeededDb {
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
        .expect("seed user");

        clients::create(
            &db,
            &ClientRow {
                id: client_id,
                name: "sm-client".into(),
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
        .expect("seed client");
    });

    SeededDb {
        db,
        user_id,
        client_id,
    }
}

// ── Oracle model ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum CodeState {
    /// Issued and available for one-time consumption.
    Active,
    /// Consumed — permanently single-use.
    Consumed,
    /// Expired without being consumed.
    Expired,
}

struct CodeOracle {
    /// Map from code index → (code plaintext, expiry, state).
    codes: Vec<(String, DateTime<Utc>, CodeState)>,
}

impl CodeOracle {
    fn new() -> Self {
        Self { codes: vec![] }
    }

    fn issue(&mut self, code: &str, expires_at: DateTime<Utc>) {
        self.codes
            .push((code.to_owned(), expires_at, CodeState::Active));
    }

    /// Oracle prediction: should a consume at `now` succeed?
    fn predict_consume(&self, idx: usize, now: DateTime<Utc>) -> bool {
        if let Some((_, exp, state)) = self.codes.get(idx) {
            *state == CodeState::Active && *exp > now
        } else {
            false
        }
    }

    /// Update oracle after a successful consume.
    fn mark_consumed(&mut self, idx: usize) {
        if let Some((_, _, state)) = self.codes.get_mut(idx) {
            *state = CodeState::Consumed;
        }
    }

    /// Oracle: expiry sweep.
    fn apply_expiry(&mut self, now: DateTime<Utc>) {
        for (_, exp, state) in &mut self.codes {
            if *state == CodeState::Active && *exp <= now {
                *state = CodeState::Expired;
            }
        }
    }
}

// ── Operation enum ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum CodeOp {
    /// Issue a new code with the given TTL in seconds (1–300).
    Issue { ttl_secs: i64 },
    /// Attempt to consume the code at index `idx % len` (if any exist).
    Consume { idx: usize },
    /// Replay: consume a code that may already be consumed.
    Replay { idx: usize },
    /// Advance the clock by `secs` seconds.
    AdvanceClock { secs: u32 },
    /// Purge expired codes.
    PurgeExpired,
}

fn code_op_strategy() -> impl Strategy<Value = CodeOp> {
    prop_oneof![
        (1i64..=300i64).prop_map(|ttl_secs| CodeOp::Issue { ttl_secs }),
        (0usize..20usize).prop_map(|idx| CodeOp::Consume { idx }),
        (0usize..20usize).prop_map(|idx| CodeOp::Replay { idx }),
        (1u32..=600u32).prop_map(|secs| CodeOp::AdvanceClock { secs }),
        Just(CodeOp::PurgeExpired),
    ]
}

// ── Harness ───────────────────────────────────────────────────────────────────

fn run_code_sequence(ops: Vec<CodeOp>) {
    let SeededDb {
        db,
        user_id,
        client_id,
    } = make_seeded_db();
    let mut oracle = CodeOracle::new();
    // Start at real now so codes with short TTLs are in the future;
    // purge_expired uses wall-clock internally and must agree.
    let mut now: DateTime<Utc> = Utc::now();
    // Track code plaintexts for lookup
    let mut code_plaintexts: Vec<String> = vec![];
    let mut code_hashes: Vec<CodeHash> = vec![];

    let rt = tokio::runtime::Runtime::new().expect("rt");

    for op in ops {
        match op {
            CodeOp::Issue { ttl_secs } => {
                let code = uuid::Uuid::new_v4().to_string();
                let hash = CodeHash::of(&code);
                let expires_at = now + Duration::seconds(ttl_secs);
                let row = AuthorizationCodeRow {
                    code_hash: hash.clone(),
                    client_id,
                    user_id,
                    redirect_uri: "https://example.com/cb".into(),
                    scope: "openid".into(),
                    nonce: None,
                    code_challenge: "challenge".into(),
                    code_challenge_method: "S256".into(),
                    expires_at,
                    consumed: false,
                    created_at: now,
                    auth_methods: vec![],
                };
                rt.block_on(auth_codes::insert(&db, &row)).expect("insert");
                oracle.issue(&code, expires_at);
                code_plaintexts.push(code);
                code_hashes.push(hash);
            }

            CodeOp::Consume { idx } | CodeOp::Replay { idx } => {
                if code_hashes.is_empty() {
                    continue;
                }
                let i = idx % code_hashes.len();
                let hash = &code_hashes[i];
                let predicted = oracle.predict_consume(i, now);

                let result = rt.block_on(auth_codes::consume(&db, hash, now));

                match (predicted, result.is_ok()) {
                    (true, true) => {
                        // INV_CODE_SINGLE_USE: oracle marks it consumed
                        oracle.mark_consumed(i);
                    }
                    (false, false) => {
                        // correct: prediction and reality both deny
                    }
                    (true, false) => {
                        panic!(
                            "INV_CODE_SINGLE_USE: oracle predicted success but consume failed for code {i}"
                        );
                    }
                    (false, true) => {
                        // INV_CODE_SINGLE_USE / INV_CODE_NO_EXPIRY_CONSUME
                        panic!(
                            "INV_CODE_SINGLE_USE / INV_CODE_NO_EXPIRY_CONSUME: consume succeeded but oracle predicted deny for code {i}"
                        );
                    }
                }
            }

            CodeOp::AdvanceClock { secs } => {
                now += Duration::seconds(secs as i64);
                // Update oracle's view of expired codes
                oracle.apply_expiry(now);
            }

            CodeOp::PurgeExpired => {
                // purge_expired uses Utc::now() internally; sync oracle to wall-clock
                // so our prediction matches what was actually purged.
                let wall_now = Utc::now();
                oracle.apply_expiry(wall_now);
                rt.block_on(auth_codes::purge_expired(&db)).expect("purge");
                // INV_CODE_PURGE_NO_RESURRECT: codes that are consumed or
                // expired must still be inaccessible after purge.
                for (i, hash) in code_hashes.iter().enumerate() {
                    if oracle.predict_consume(i, now) {
                        // Still active — not relevant to purge invariant.
                        continue;
                    }
                    // Non-active (consumed or expired): must not succeed.
                    let result = rt.block_on(auth_codes::consume(&db, hash, now));
                    assert!(
                        result.is_err(),
                        "INV_CODE_PURGE_NO_RESURRECT: non-active code {i} was consumable after purge"
                    );
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

    /// State-machine property: auth code lifecycle invariants hold across
    /// any sequence of issue / consume / replay / clock-advance / purge.
    ///
    /// Named invariants checked:
    /// - `INV_CODE_SINGLE_USE`
    /// - `INV_CODE_NO_EXPIRY_CONSUME`
    /// - `INV_CODE_PURGE_NO_RESURRECT`
    #[test]
    fn auth_code_lifecycle_invariants(
        ops in proptest::collection::vec(code_op_strategy(), 1..=40)
    ) {
        run_code_sequence(ops);
    }
}
