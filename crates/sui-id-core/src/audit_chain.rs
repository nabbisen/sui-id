//! Whether the audit-log hash chain verified, and what that means (RFC 121).
//!
//! Before this module, each of the three places that called
//! `verify_chain_tail` decided for itself what an `Err` meant: the audit page
//! substituted a report meaning "nothing wrong"; the settings page failed its
//! whole tab; startup logged a `warn!` that a broken chain would have gotten
//! an `error!`. [`check`] is the one function all three now call instead, so
//! none of them can re-decide, forget, or disagree with another surface about
//! what happened.
//!
//! [`check`] always returns one of exactly three outcomes ([`ChainCheck`]) —
//! never a default standing in for one it did not obtain — and it always
//! records a failure to verify before returning, unconditionally, so no
//! caller can bypass that by only rendering what it likes.

use crate::time::SharedClock;
use sui_id_store::Database;
use sui_id_store::models::AuditLogRow;
use sui_id_store::registry;
use sui_id_store::repos::audit;

/// The result of one [`check`] call, turned into exactly the three outcomes
/// RFC 121 D1 names. Every variant carries `limit`: "intact" and "broken"
/// both mean something only inside the window they were checked against —
/// three different callers use three different limits, and a result is not
/// safe to read without knowing which.
#[derive(Debug, Clone)]
pub enum ChainCheck {
    /// The most recent `limit` rows verified as a chain: linked, sequential,
    /// and each row's own hash correct. `legacy_unhashed` counts rows that
    /// predate hashing (RFC 125 stage 2 — a nonzero value can only mean
    /// genuine pre-v0.17.0 rows; it is shown for that reason, not withheld).
    Intact {
        checked: usize,
        legacy_unhashed: usize,
        limit: i64,
    },
    /// A row within the checked window failed one of `verify_chain_tail`'s
    /// checks (its own hash, its linkage to the row before it, sequence
    /// continuity, or the legacy-prefix rule). `at_seq` names the row.
    Broken {
        at_seq: i64,
        checked: usize,
        legacy_unhashed: usize,
        limit: i64,
    },
    /// Verification did not complete. This is an unknown, not a known-good
    /// or a known-bad result, and no surface may read it as either
    /// (RFC 121 D1, D2). `detail` is the store error's own message.
    CouldNotVerify { detail: String, limit: i64 },
}

impl ChainCheck {
    /// The window this result was checked against, whichever outcome it is.
    pub fn limit(&self) -> i64 {
        match self {
            Self::Intact { limit, .. } | Self::Broken { limit, .. } => *limit,
            Self::CouldNotVerify { limit, .. } => *limit,
        }
    }
}

/// Verify the most recent `limit` audit rows as a chain, and record the
/// result before returning it.
///
/// **The record does not depend on the thing that might have failed
/// (RFC 121 D3).** A failure to verify is logged via `tracing::error!` —
/// that channel needs nothing from the store — and, only as a secondary,
/// best-effort signal, as an audit-log row: `append` uses the same
/// connection that just failed to be read, so a store-wide cause fails both,
/// and if the cause is the very tampering this system exists to catch, a row
/// recording "I could not read myself" would live in the thing that is the
/// problem. It is attempted anyway, on the reasoning that a transient
/// failure — by far the more common cause — is worth a durable trace, and
/// never placed on the critical path: its own failure is silently discarded,
/// exactly like every other best-effort audit write in this codebase.
///
/// A broken chain and an intact one are each logged too (`error!` and
/// `info!`), at levels that order correctly relative to a failure to verify
/// (D2): "could not verify" is not less severe than "broken", so both are
/// `error!`, and only "intact" is `info!`. None of the three outcomes stops
/// the caller — that decision is the caller's (RFC 121 D4 for startup).
pub async fn check(db: &Database, clock: &SharedClock, limit: i64) -> ChainCheck {
    let result = match audit::verify_chain_tail(db, limit).await {
        Ok(report) => match report.broken_at_seq {
            Some(at_seq) => ChainCheck::Broken {
                at_seq,
                checked: report.checked,
                legacy_unhashed: report.legacy_unhashed,
                limit,
            },
            None => ChainCheck::Intact {
                checked: report.checked,
                legacy_unhashed: report.legacy_unhashed,
                limit,
            },
        },
        Err(e) => ChainCheck::CouldNotVerify {
            detail: e.to_string(),
            limit,
        },
    };
    record(db, clock, &result).await;
    result
}

async fn record(db: &Database, clock: &SharedClock, check: &ChainCheck) {
    match check {
        ChainCheck::Broken {
            at_seq,
            checked,
            legacy_unhashed,
            limit,
        } => {
            tracing::error!(
                broken_at_seq = at_seq,
                checked,
                legacy_unhashed,
                limit,
                "audit-log hash-chain verification FAILED — tampering or DB corruption suspected"
            );
        }
        ChainCheck::CouldNotVerify { detail, limit } => {
            tracing::error!(
                detail = %detail,
                limit,
                "audit-log chain verification could not run"
            );
            // Best-effort, and deliberately not `?`: see this module's doc
            // and `check`'s for why a failure here is discarded rather than
            // propagated or retried.
            //
            // The note is built through the RFC 105 encoder, not `format!`
            // — `detail` is a store error's message, and a future
            // `StoreError` variant (several already interpolate arbitrary
            // content, e.g. `Integrity(String)`) could put a forged
            // `key=value` pair inside it. `render_note` percent-encodes
            // every value, so a pair inside `detail` cannot be read back as
            // a real attribute (RFC 105's own guarantee — see
            // `sui_id_store::registry`'s module doc). If the attributes
            // fail to build at all (e.g. `detail` alone exceeding the
            // per-attribute byte bound), the event is still worth a row
            // without a note rather than not at all.
            let note = registry::AuditAttributes::builder()
                .attribute("limit", limit.to_string())
                .attribute("detail", detail.clone())
                .build()
                .ok()
                .and_then(|attrs| registry::render_note(&attrs));
            let _ = audit::append(
                db,
                &AuditLogRow {
                    at: clock.now(),
                    actor: None,
                    action: "audit.chain.verification_failed".into(),
                    target: None,
                    result: "error".into(),
                    note,
                },
            )
            .await;
        }
        ChainCheck::Intact {
            checked,
            legacy_unhashed,
            limit,
        } => {
            tracing::info!(
                checked,
                legacy_unhashed,
                limit,
                "audit-log hash chain verified"
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::time::system_clock;
    use sui_id_shared::ids::UserId;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::AuditLogRow;

    fn fresh_db() -> Database {
        Database::open_in_memory(MasterKey::generate()).expect("db")
    }

    fn sample_row(action: &str) -> AuditLogRow {
        AuditLogRow {
            at: chrono::Utc::now(),
            actor: Some(UserId::new()),
            action: action.into(),
            target: Some("target-x".into()),
            result: "ok".into(),
            note: None,
        }
    }

    #[tokio::test]
    async fn an_empty_chain_is_intact() {
        let db = fresh_db();
        let clock = system_clock();
        let r = check(&db, &clock, 100).await;
        assert!(matches!(
            r,
            ChainCheck::Intact {
                checked: 0,
                legacy_unhashed: 0,
                limit: 100
            }
        ));
        assert_eq!(r.limit(), 100);
    }

    #[tokio::test]
    async fn a_consistent_chain_is_intact() {
        let db = fresh_db();
        let clock = system_clock();
        audit::append(&db, &sample_row("a")).await.expect("append");
        audit::append(&db, &sample_row("b")).await.expect("append");
        let r = check(&db, &clock, 100).await;
        match r {
            ChainCheck::Intact { checked, .. } => assert_eq!(checked, 2),
            other => panic!("expected Intact, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_broken_chain_names_the_row_and_carries_the_limit() {
        let db = fresh_db();
        let clock = system_clock();
        audit::append(&db, &sample_row("a")).await.expect("append");
        audit::append(&db, &sample_row("b")).await.expect("append");
        db.with_conn(|c| {
            c.execute("UPDATE audit_log SET action = 'tampered' WHERE seq = 1", [])?;
            Ok(())
        })
        .await
        .expect("tamper");
        let r = check(&db, &clock, 100).await;
        match r {
            ChainCheck::Broken { at_seq, limit, .. } => {
                assert_eq!(at_seq, 1);
                assert_eq!(limit, 100);
            }
            other => panic!("expected Broken, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_read_error_is_could_not_verify_not_intact_and_not_broken() {
        let db = fresh_db();
        let clock = system_clock();
        audit::append(&db, &sample_row("a")).await.expect("append");
        db.with_conn(|c| {
            c.execute(
                "UPDATE audit_log SET actor = 'not-a-uuid' WHERE seq = 1",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("corrupt actor");
        let r = check(&db, &clock, 100).await;
        assert!(
            matches!(r, ChainCheck::CouldNotVerify { .. }),
            "expected CouldNotVerify, got {r:?}"
        );
        assert_eq!(r.limit(), 100);
    }

    #[tokio::test]
    async fn a_could_not_verify_result_writes_a_best_effort_audit_row() {
        let db = fresh_db();
        let clock = system_clock();
        audit::append(&db, &sample_row("a")).await.expect("append");
        db.with_conn(|c| {
            c.execute(
                "UPDATE audit_log SET actor = 'not-a-uuid' WHERE seq = 1",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("corrupt actor");
        let _ = check(&db, &clock, 100).await;
        // Read the row directly: `verify_chain_tail` itself cannot be used to
        // observe it, since the corrupted actor value makes every call
        // through it fail the same way.
        let rows: Vec<String> = db
            .with_conn(|c| {
                let mut stmt = c.prepare(
                    "SELECT action FROM audit_log WHERE action = 'audit.chain.verification_failed'",
                )?;
                let rows = stmt
                    .query_map([], |r| r.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .expect("read");
        assert_eq!(rows.len(), 1, "expected exactly one recorded failure row");
    }

    #[tokio::test]
    async fn a_could_not_verify_notes_detail_encoded_so_a_forged_pair_does_not_parse() {
        // RFC 121 review: `detail` is a store error's message, and a future
        // `StoreError` variant could put arbitrary content — including a
        // forged `key=value` pair — inside it. Calling `record` directly
        // with a `detail` shaped exactly like that attack lets this test
        // control the content precisely, independent of what any real
        // `StoreError` happens to produce today.
        let db = fresh_db();
        let clock = system_clock();
        let forged = ChainCheck::CouldNotVerify {
            detail: "forged via=web trailing".into(),
            limit: 42,
        };
        record(&db, &clock, &forged).await;

        let note: String = db
            .with_conn(|c| {
                c.query_row(
                    "SELECT note FROM audit_log WHERE action = 'audit.chain.verification_failed'",
                    [],
                    |r| r.get(0),
                )
                .map_err(Into::into)
            })
            .await
            .expect("read note");

        // The forged pair is not readable as a real attribute: `via` was
        // never a key this event declares, and encoding stops it from
        // being read as one anyway.
        assert_eq!(registry::note_field(&note, "via"), None, "{note}");
        // The real attribute round-trips exactly, space and `=` intact.
        assert_eq!(
            registry::note_field(&note, "detail").as_deref(),
            Some("forged via=web trailing"),
            "{note}"
        );
        assert_eq!(
            registry::note_field(&note, "limit").as_deref(),
            Some("42"),
            "{note}"
        );
    }

    #[tokio::test]
    async fn an_intact_result_writes_no_extra_audit_row() {
        let db = fresh_db();
        let clock = system_clock();
        audit::append(&db, &sample_row("a")).await.expect("append");
        let before = db
            .with_conn(|c| {
                c.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get::<_, i64>(0))
                    .map_err(Into::into)
            })
            .await
            .expect("count");
        let _ = check(&db, &clock, 100).await;
        let after = db
            .with_conn(|c| {
                c.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get::<_, i64>(0))
                    .map_err(Into::into)
            })
            .await
            .expect("count");
        assert_eq!(before, after);
    }
}
