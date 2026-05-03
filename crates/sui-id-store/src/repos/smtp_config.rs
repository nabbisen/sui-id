//! `smtp_config` table — singleton row, see migration 0014.
//!
//! The row is keyed on the literal string `'singleton'`. Reads use
//! `get()` (returns `Option`); writes use `upsert()` (insert or
//! update by primary key).

use crate::{
    crypto::{open, seal, MasterKey},
    models::{SmtpConfigRow, SmtpTlsMode},
    Database, StoreError, StoreResult,
};
use chrono::{DateTime, Utc};
use rusqlite::params;

const SINGLETON_ID: &str = "singleton";

/// AAD for the password seal/open. Distinct from any other column
/// AAD in the schema, so a sealed value cannot be cross-fed to a
/// different column even if the master key were misused.
pub const SMTP_PASSWORD_AAD: &[u8] = b"smtp.password";

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SmtpConfigRow> {
    let tls_mode_str: String = row.get(3)?;
    let tls_mode = SmtpTlsMode::parse(&tls_mode_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            Box::new(StoreError::Integrity(format!(
                "unknown smtp_config.tls_mode value: {tls_mode_str}"
            ))),
        )
    })?;
    Ok(SmtpConfigRow {
        enabled: row.get::<_, i64>(1)? != 0,
        host: row.get(2)?,
        port: row.get::<_, i64>(4)? as u16,
        tls_mode,
        username: row.get(5)?,
        password_enc: row.get::<_, Option<Vec<u8>>>(6)?,
        from_address: row.get(7)?,
        from_name: row.get(8)?,
        base_url: row.get(9)?,
        created_at: row.get::<_, DateTime<Utc>>(10)?,
        updated_at: row.get::<_, DateTime<Utc>>(11)?,
    })
}

const SELECT_COLUMNS: &str = "id, enabled, host, tls_mode, port, username, \
                              password_enc, from_address, from_name, base_url, \
                              created_at, updated_at";

/// Fetch the current SMTP configuration, if any.
pub fn get(db: &Database) -> StoreResult<Option<SmtpConfigRow>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM smtp_config WHERE id = ?1"
        ))?;
        let res = stmt.query_row([SINGLETON_ID], map_row);
        match res {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    })
}

/// Insert or update the singleton SMTP configuration row.
///
/// Always writes `id = 'singleton'`; the row's primary key is
/// fixed by the schema's CHECK constraint.
pub fn upsert(db: &Database, row: &SmtpConfigRow) -> StoreResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO smtp_config(id, enabled, host, port, tls_mode, username, \
                                      password_enc, from_address, from_name, base_url, \
                                      created_at, updated_at) \
             VALUES('singleton', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
             ON CONFLICT(id) DO UPDATE SET \
                enabled = excluded.enabled, \
                host = excluded.host, \
                port = excluded.port, \
                tls_mode = excluded.tls_mode, \
                username = excluded.username, \
                password_enc = excluded.password_enc, \
                from_address = excluded.from_address, \
                from_name = excluded.from_name, \
                base_url = excluded.base_url, \
                updated_at = excluded.updated_at",
            params![
                row.enabled as i64,
                row.host,
                row.port as i64,
                row.tls_mode.as_str(),
                row.username,
                row.password_enc,
                row.from_address,
                row.from_name,
                row.base_url,
                row.created_at,
                row.updated_at,
            ],
        )?;
        Ok(())
    })
}

/// Decrypt the SMTP password using the supplied master key.
/// Returns `None` if the row has no password set (relay does
/// not require authentication).
pub fn decrypt_password(
    row: &SmtpConfigRow,
    master_key: &MasterKey,
) -> StoreResult<Option<String>> {
    let Some(ct) = row.password_enc.as_deref() else {
        return Ok(None);
    };
    let plaintext = open(master_key, ct, SMTP_PASSWORD_AAD)?;
    let s = String::from_utf8(plaintext).map_err(|_| StoreError::Crypto)?;
    Ok(Some(s))
}

/// Seal an SMTP password for storage. Returns the ciphertext bytes
/// suitable for `SmtpConfigRow.password_enc`.
pub fn seal_password(plaintext: &str, master_key: &MasterKey) -> StoreResult<Vec<u8>> {
    seal(master_key, plaintext.as_bytes(), SMTP_PASSWORD_AAD)
}

/// Re-seal the singleton `password_enc` (if present) under
/// `new_key`. Returns 1 if a password was re-sealed, 0 if none
/// is configured. Used by master-key rotation; does not commit.
pub fn reseal_all(
    tx: &rusqlite::Transaction<'_>,
    old_key: &crate::crypto::MasterKey,
    new_key: &crate::crypto::MasterKey,
) -> StoreResult<u64> {
    let row: Option<Vec<u8>> = tx
        .query_row(
            "SELECT password_enc FROM smtp_config WHERE id = 'singleton'",
            [],
            |r| r.get::<_, Option<Vec<u8>>>(0),
        )
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    let Some(enc) = row else {
        return Ok(0);
    };
    let plain = crate::crypto::open(old_key, &enc, SMTP_PASSWORD_AAD)?;
    let resealed = crate::crypto::seal(new_key, &plain, SMTP_PASSWORD_AAD)?;
    tx.execute(
        "UPDATE smtp_config SET password_enc = ?1 WHERE id = 'singleton'",
        rusqlite::params![resealed],
    )?;
    Ok(1)
}
