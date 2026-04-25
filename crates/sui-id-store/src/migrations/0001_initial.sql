-- sui-id initial schema. Migration 0001.
--
-- Conventions:
--   * Identifiers are stored as TEXT holding a UUID v4 string.
--   * Timestamps are stored as TEXT in RFC 3339 (UTC).
--   * Columns whose names end in `_enc` hold an XChaCha20-Poly1305 sealed blob
--     (nonce || ciphertext || tag). Their plaintext shape is documented per
--     repository.
--   * `is_disabled` is the soft-disable / suspension flag.
--   * `is_deleted` is the logical-delete flag.

PRAGMA foreign_keys = ON;

-- ----- internal metadata --------------------------------------------------
CREATE TABLE IF NOT EXISTS sui_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- ----- users ---------------------------------------------------------------
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    display_name TEXT,
    is_admin INTEGER NOT NULL DEFAULT 0,
    is_disabled INTEGER NOT NULL DEFAULT 0,
    is_deleted INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- ----- credentials (one-to-one with users) --------------------------------
CREATE TABLE IF NOT EXISTS credentials (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- Argon2id PHC-formatted hash (already opaque/non-reversible; not encrypted).
    password_hash TEXT NOT NULL,
    -- For future use: when we want to force re-auth without rotating the row.
    must_change INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

-- ----- clients (relying parties) ------------------------------------------
CREATE TABLE IF NOT EXISTS clients (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    confidential INTEGER NOT NULL,
    -- Argon2id hash of the client secret. NULL for public clients.
    secret_hash TEXT,
    redirect_uris TEXT NOT NULL,   -- JSON array of strings
    is_disabled INTEGER NOT NULL DEFAULT 0,
    is_deleted INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- ----- authorization codes (single-use, short-lived) ----------------------
CREATE TABLE IF NOT EXISTS auth_codes (
    code_hash TEXT PRIMARY KEY,    -- SHA-256 of the issued code
    client_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    scope TEXT NOT NULL,
    nonce TEXT,
    code_challenge TEXT NOT NULL,
    code_challenge_method TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    consumed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_auth_codes_expires ON auth_codes(expires_at);

-- ----- sessions (admin login) ---------------------------------------------
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    revoked_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);

-- ----- refresh tokens ------------------------------------------------------
CREATE TABLE IF NOT EXISTS refresh_tokens (
    id TEXT PRIMARY KEY,
    -- The token itself is sealed at rest (the user only ever sees the
    -- plaintext returned at issuance, never read back from the DB).
    token_enc BLOB NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    scope TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    revoked_at TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user ON refresh_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_client ON refresh_tokens(client_id);

-- ----- signing keys --------------------------------------------------------
CREATE TABLE IF NOT EXISTS signing_keys (
    id TEXT PRIMARY KEY,
    algorithm TEXT NOT NULL,           -- e.g. "EdDSA"
    private_key_enc BLOB NOT NULL,     -- sealed PKCS#8 / raw bytes
    public_key BLOB NOT NULL,          -- raw 32-byte Ed25519 public key
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    rotated_at TEXT
);

-- ----- consent (placeholder for future expansion) -------------------------
CREATE TABLE IF NOT EXISTS consents (
    user_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    granted_at TEXT NOT NULL,
    PRIMARY KEY(user_id, client_id)
);

-- ----- audit log -----------------------------------------------------------
-- Append-only; no UPDATE / DELETE statements are issued by the codebase.
CREATE TABLE IF NOT EXISTS audit_log (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    at TEXT NOT NULL,
    actor TEXT,
    action TEXT NOT NULL,
    target TEXT,
    result TEXT NOT NULL,
    note TEXT
);
CREATE INDEX IF NOT EXISTS idx_audit_log_at ON audit_log(at);
