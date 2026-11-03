//! Master-key re-seal: a pure, database-free helper.
//!
//! The actual re-seal orchestration (open under the old key, re-seal
//! six tables in one transaction, append the audit record in the same
//! transaction) moved to `sui_id_store::commands::reseal_master_key`
//! (K07) in RFC 094 cycle J stage 4 — `declare_write_command!` is
//! defined in `sui-id-store`, which this crate depends on, not the
//! reverse, so the sealed command could only live there. What remains
//! here is [`reseal_one`]: a per-ciphertext helper with no production
//! caller (the per-table `reseal_all` functions in `sui-id-store`'s
//! repos re-seal directly, not through this), kept because it is
//! unit-testable without a database and nothing in the dispatch that
//! moved its caller asked for it to go too.
//!
//! The CLI subcommand is still `sui-id admin rotate-key`; the old key
//! file is renamed to `<name>.bak.<ISO timestamp>` by `cli.rs` after
//! K07 commits. See `docs/src/guides/operators.md`'s "Rotating the
//! master key" section for the operator-facing procedure.

use crate::errors::{CoreError, CoreResult};
use sui_id_store::crypto::{MasterKey, open, seal};

/// Pure helper: re-seal a single ciphertext under `new_key`,
/// preserving the AAD. No production caller — the per-table
/// `reseal_all` functions in `sui-id-store`'s repos re-seal directly,
/// not through this; it lives here so the primitive is unit-testable
/// without requiring a database.
pub async fn reseal_one(
    old_key: &MasterKey,
    new_key: &MasterKey,
    sealed: &[u8],
    aad: &[u8],
) -> CoreResult<Vec<u8>> {
    let plaintext = open(old_key, sealed, aad)
        .map_err(|_| CoreError::BadRequest("decrypt with old key failed".into()))?;
    let resealed = seal(new_key, &plaintext, aad).map_err(|_| CoreError::Internal)?;
    Ok(resealed)
}

#[cfg(test)]
#[path = "key_rotation/tests.rs"]
mod tests;
