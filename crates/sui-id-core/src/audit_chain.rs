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
mod tests;
