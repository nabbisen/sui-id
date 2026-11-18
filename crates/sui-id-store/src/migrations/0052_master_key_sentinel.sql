-- RFC 100 stage 1: the startup key check.
--
-- `master_key_sentinel` is a singleton row holding a small, fixed
-- plaintext sealed under the active master key — checked on every start
-- and by every CLI command that opens the database, so a wrong key is
-- refused before readiness instead of being discovered later, by chance,
-- at the first signing-key or token operation that happens to touch a
-- sealed column. `sealed` is the XChaCha20-Poly1305 envelope
-- (`crypto::seal`'s own `nonce(24) || ciphertext || tag(16)` layout, as
-- every other `_enc`/`_sealed` column in this schema uses it);
-- `key_fingerprint` is the non-secret HMAC-based fingerprint (RFC 100 §2)
-- of the key that sealed it, stored alongside so a refusal can name it
-- without decrypting anything.
--
-- `id INTEGER CHECK (id = 1)` is this schema's established singleton
-- idiom where the key is numeric rather than a fixed string (compare
-- `smtp_config`'s `id = 'singleton'` TEXT key, migration 0014) — there is
-- exactly one row, ever, and an attempt to insert a second violates the
-- `PRIMARY KEY` before it reaches this `CHECK`.
CREATE TABLE IF NOT EXISTS master_key_sentinel (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    sealed          BLOB    NOT NULL,
    key_fingerprint TEXT    NOT NULL,
    written_at      TEXT    NOT NULL
) STRICT;

-- `master_key_rotation`: unused until stage 6 (K07 extended, activation,
-- K05), added here because both tables are this stage's migration per
-- the implementation plan. A singleton row, the same idiom: at most one
-- rotation is ever in flight or most-recently-completed. `state` is
-- forward-only, the same convention `federation_login_attempt` (migration
-- 0046) uses for its own status column — nothing in this stage writes to
-- this table; stage 6 does.
CREATE TABLE IF NOT EXISTS master_key_rotation (
    id                   INTEGER PRIMARY KEY CHECK (id = 1),
    rotation_id          TEXT    NOT NULL,
    old_fingerprint      TEXT    NOT NULL,
    new_fingerprint      TEXT    NOT NULL,
    state                TEXT    NOT NULL
                             CHECK (state IN ('db_committed_pending_activation', 'complete')),
    started_at           TEXT    NOT NULL,
    updated_at           TEXT    NOT NULL,
    backup_filename      TEXT    NOT NULL,
    custody_disposition  TEXT    NOT NULL
) STRICT;
