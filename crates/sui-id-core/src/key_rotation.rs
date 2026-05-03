//! Master-key rotation.
//!
//! Re-seal every encrypted column in the database under a new
//! 32-byte XChaCha20-Poly1305 master key. The operation runs
//! offline: the server is stopped, the operator runs the
//! `sui-id admin rotate-key` CLI, the old key file is renamed
//! to `<name>.bak.<ISO timestamp>` (kept beside the new one so
//! recovery from a backup is straightforward), and the server
//! is restarted with the new key.
//!
//! ## Why offline
//!
//! Hot rotation (live re-key while the server is running) was
//! evaluated and rejected. The complexity ladder is steep — every
//! sealed read needs to fall back through the old key, every
//! seal needs to choose the new one, the cutover has to be
//! globally consistent, and the failure modes (partial rotation
//! after a crash) require new state machinery to recover from.
//! Offline rotation gives sui-id the strongest guarantee — every
//! row is either fully old-keyed or fully new-keyed at any
//! point an operator can observe — at the cost of a few seconds
//! of downtime once or twice in the lifetime of a deployment.
//!
//! ## Atomicity
//!
//! All re-seals run inside a single SQLite transaction. On any
//! error during the loop, the transaction rolls back: the DB
//! file remains entirely under the old key, the old key file
//! has not yet been renamed (the rename happens AFTER COMMIT),
//! and a re-run with the same arguments is a clean retry. There
//! is no half-rotated state to recover from.
//!
//! ## Old-key preservation
//!
//! After the transaction commits, the old key file is renamed
//! to `<original_path>.bak.<RFC3339 timestamp>`. This:
//!
//! - keeps the old material available for restoring from a
//!   pre-rotation DB backup (the rotation itself does not back
//!   up the DB — that's an operator responsibility);
//! - moves the old file out of the path the server reads from
//!   on next startup, so the server picks up the new key
//!   without further configuration changes;
//! - leaves the old file in the same directory the operator
//!   already manages permissions on, rather than scattering
//!   secrets across the filesystem.
//!
//! Old key files are not auto-deleted. The operator decides
//! when (or whether) to remove them.

use crate::errors::{CoreError, CoreResult};
use sui_id_store::crypto::{open, seal, MasterKey};
use sui_id_store::repos::{
    refresh_tokens, signing_keys, smtp_config, user_totp, user_webauthn_credentials,
};
use sui_id_store::Database;

/// Result of running a rotation. Counts of re-sealed rows in
/// each table — useful for the CLI to print and for tests to
/// assert on.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RotationReport {
    pub signing_keys: u64,
    pub refresh_tokens: u64,
    pub user_totp_secrets: u64,
    pub user_totp_recovery_codes: u64,
    pub user_webauthn_credentials: u64,
    pub smtp_config: u64,
}

impl RotationReport {
    pub fn total(&self) -> u64 {
        self.signing_keys
            + self.refresh_tokens
            + self.user_totp_secrets
            + self.user_totp_recovery_codes
            + self.user_webauthn_credentials
            + self.smtp_config
    }
}

/// Re-seal every encrypted column in the database under
/// `new_key`. Runs in a single SQLite transaction; on any error
/// the transaction is rolled back and `Err(_)` is returned with
/// no observable change to the DB.
///
/// The caller is responsible for:
///
/// - taking a fresh backup of the DB *before* calling this
///   (one rough alignment of the `backup` skill is plenty);
/// - opening `db` under the **old** key (so reads succeed);
/// - having the new key already constructed, ideally from
///   `MasterKey::generate` or `from_base64`;
/// - renaming the old key file out of the way after this
///   function returns successfully (the CLI does this).
pub fn rotate_master_key(
    db: &Database,
    new_key: &MasterKey,
) -> CoreResult<RotationReport> {
    let old_key = db.key();
    let mut report = RotationReport::default();

    // Each table has its own SELECT/UPDATE shape (different AADs,
    // different number of sealed columns), and we want to error
    // out early on the first failure with the rest still intact.
    // SQLite-level transactionality is provided by `Database::with_conn`
    // running inside an implicit transaction that rolls back on Err.
    db.with_tx(|tx| {
        report.signing_keys = signing_keys::reseal_all(tx, old_key, new_key)?;
        report.refresh_tokens = refresh_tokens::reseal_all(tx, old_key, new_key)?;
        let (totp_n, recovery_n) = user_totp::reseal_all(tx, old_key, new_key)?;
        report.user_totp_secrets = totp_n;
        report.user_totp_recovery_codes = recovery_n;
        report.user_webauthn_credentials =
            user_webauthn_credentials::reseal_all(tx, old_key, new_key)?;
        report.smtp_config = smtp_config::reseal_all(tx, old_key, new_key)?;
        Ok(())
    })?;

    Ok(report)
}

/// Pure helper: re-seal a single ciphertext under `new_key`,
/// preserving the AAD. Implementation detail of the per-table
/// `reseal_all` functions; lives here so it is unit-testable
/// without requiring a database.
pub fn reseal_one(
    old_key: &MasterKey,
    new_key: &MasterKey,
    sealed: &[u8],
    aad: &[u8],
) -> CoreResult<Vec<u8>> {
    let plaintext = open(old_key, sealed, aad)
        .map_err(|_| CoreError::BadRequest("decrypt with old key failed".into()))?;
    let resealed = seal(new_key, &plaintext, aad)
        .map_err(|_| CoreError::Internal)?;
    Ok(resealed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_id_store::crypto::MasterKey;

    #[test]
    fn reseal_one_round_trip() {
        let old = MasterKey::generate();
        let new = MasterKey::generate();
        let aad = b"test-aad";
        let plaintext = b"hello world".to_vec();
        let sealed = seal(&old, &plaintext, aad).expect("seal");
        let resealed = reseal_one(&old, &new, &sealed, aad).expect("reseal");
        // The re-sealed ciphertext must decrypt under the NEW key,
        // and must NOT decrypt under the OLD key.
        let opened_new = open(&new, &resealed, aad).expect("open with new");
        assert_eq!(opened_new, plaintext);
        assert!(open(&old, &resealed, aad).is_err());
    }

    #[test]
    fn reseal_one_fails_with_wrong_old_key() {
        let real_old = MasterKey::generate();
        let wrong_old = MasterKey::generate();
        let new = MasterKey::generate();
        let aad = b"test-aad";
        let sealed = seal(&real_old, b"data", aad).expect("seal");
        // Wrong "old" key: open should fail, error propagates.
        assert!(reseal_one(&wrong_old, &new, &sealed, aad).is_err());
    }

    #[test]
    fn reseal_one_with_wrong_aad_fails() {
        let old = MasterKey::generate();
        let new = MasterKey::generate();
        let sealed = seal(&old, b"data", b"correct-aad").expect("seal");
        assert!(reseal_one(&old, &new, &sealed, b"wrong-aad").is_err());
    }

    #[test]
    fn rotation_report_total_sums_columns() {
        let r = RotationReport {
            signing_keys: 1,
            refresh_tokens: 5,
            user_totp_secrets: 3,
            user_totp_recovery_codes: 3,
            user_webauthn_credentials: 2,
            smtp_config: 1,
        };
        assert_eq!(r.total(), 15);
    }
}
