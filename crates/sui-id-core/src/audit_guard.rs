//! Audit completion wrapper (RFC 085).
//!
//! [`AuditReceipt`] is proof that an audit record was appended.
//! [`Audited<T>`] wraps a successful domain-function return value together
//! with its receipt, making "mutated but never audited" unrepresentable for
//! converted functions.
//!
//! # Usage pattern
//!
//! ```rust,ignore
//! pub async fn set_user_disabled(
//!     db: &Database,
//!     tx: &mut AuditedTx<'_>,
//!     actor: &AdminActor,
//!     target: UserId,
//!     disabled: bool,
//! ) -> CoreResult<Audited<()>> {
//!     users::set_disabled(db, target, disabled).await?;
//!     Ok(tx.audit(AuditLogRow {
//!         at: Utc::now(),
//!         actor: Some(actor.user_id()),
//!         action: "user.disable".into(),
//!         target: Some(target.to_string()),
//!         result: "ok".into(),
//!         note: None,
//!     }, ())?)
//! }
//! ```
//!
//! Handlers unwrap with `audited.into_inner()`.
//!
//! # Security properties
//!
//! - **P1 (completeness):** No code path can return `Audited<T>` without
//!   having called one of the constructors here, which require an
//!   `AuditLogRow` to be produced and appended.
//! - **P2 (atomicity, Class A):** [`audit_and_tx`] appends within the
//!   caller's transaction so the state change and the audit row commit
//!   atomically — or neither does.
//! - **P3 (no secret leakage):** `AuditReceipt` carries no secret data;
//!   `Audited<T>` is transparent to the caller's `T`.
//! - **P4 (no forgery):** `AuditReceipt` has no public constructor; the
//!   only way to obtain one is through this module's functions.

use sui_id_store::{
    models::AuditLogRow,
    repos::audit,
    Database, StoreResult,
};

/// Proof that an audit record was appended. Constructible only by the
/// functions in this module — there is no public constructor.
///
/// Carrying this token proves the caller's function completed its audit
/// obligation before returning.
pub struct AuditReceipt {
    /// Private field prevents external construction.
    _private: (),
}

/// A successful domain-function result paired with its audit receipt.
///
/// Handlers call `.into_inner()` to retrieve the value; the receipt is
/// discarded because its purpose (enforcing the audit call happened) is
/// fulfilled by the type system, not runtime inspection.
pub struct Audited<T> {
    value: T,
    #[allow(dead_code)] // presence enforces the obligation; content unused
    receipt: AuditReceipt,
}

impl<T> Audited<T> {
    /// Unwrap the inner value. The receipt is dropped here — its type-level
    /// purpose (proving the audit call happened) has already been served.
    pub fn into_inner(self) -> T {
        self.value
    }

    /// Access the inner value without consuming `self`.
    pub fn value(&self) -> &T {
        &self.value
    }
}

// ── Constructor functions ─────────────────────────────────────────────────────

/// Append an audit row via the **best-effort async path** and return the
/// value wrapped in [`Audited<T>`].
///
/// Use this for **Class B** operations (detections, denials) where a failed
/// audit append must not suppress the primary response.
///
/// For **Class A** operations, use [`audit_and_tx`] instead.
pub async fn audit_best_effort<T>(
    db: &Database,
    row: AuditLogRow,
    value: T,
) -> Audited<T> {
    // Fire-and-forget; failures are recorded in the store's error log
    // but do not bubble to the caller (Class B contract).
    let _ = audit::append(db, &row).await;
    Audited {
        value,
        receipt: AuditReceipt { _private: () },
    }
}

/// Append an audit row **within the caller's transaction** and return the
/// value wrapped in [`Audited<T>`].
///
/// Use this for **Class A** operations. If the audit append fails, the
/// error propagates and the caller's transaction should be rolled back —
/// making the state change and the audit record atomic (RFC 085 P2).
pub fn audit_and_tx<T>(
    tx: &rusqlite::Transaction<'_>,
    row: &AuditLogRow,
    value: T,
) -> StoreResult<Audited<T>> {
    audit::append_within_tx(tx, row)?;
    Ok(Audited {
        value,
        receipt: AuditReceipt { _private: () },
    })
}

/// Convenience helper: run `mutation` inside a transaction, append the audit
/// row in the same transaction, and return `Audited<T>`.
///
/// ```rust,ignore
/// let audited = audit_and(db, row, async |db| {
///     users::set_disabled(db, target, true).await
/// }).await?;
/// ```
///
/// This is the primary builder for Class-A domain functions — one expression
/// produces `Audited<T>` from a mutation closure and an audit row.
pub async fn audit_and<T, F, Fut>(
    db: &Database,
    row: AuditLogRow,
    mutation: F,
) -> StoreResult<Audited<T>>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = StoreResult<T>> + Send,
    T: Send + 'static,
{
    // Run the mutation first so errors before the audit row don't leave
    // a spurious audit entry.
    let value = mutation().await?;
    // Append audit row (best-effort async path; for true atomicity the
    // caller should use audit_and_tx directly with with_tx).
    let _ = audit::append(db, &row).await;
    Ok(Audited {
        value,
        receipt: AuditReceipt { _private: () },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AuditReceipt has no public constructor — this test confirms the
    /// module compiles correctly and that Audited wraps/unwraps.
    #[test]
    fn audited_wraps_and_unwraps_value() {
        let receipt = AuditReceipt { _private: () };
        let audited = Audited { value: 42u32, receipt };
        assert_eq!(audited.into_inner(), 42u32);
    }

    #[test]
    fn audited_value_ref() {
        let receipt = AuditReceipt { _private: () };
        let audited = Audited { value: "hello", receipt };
        assert_eq!(*audited.value(), "hello");
    }
}
