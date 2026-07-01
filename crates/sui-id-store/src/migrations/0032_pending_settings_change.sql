-- RFC 090: pending_settings_change table.
--
-- Stores encrypted, session-bound, expiring pending changes for high-risk
-- settings that include secrets (e.g. SMTP password).  The confirm page
-- carries only `id` (the pending_change_id) plus a non-secret summary;
-- the actual payload is encrypted with the master key.
--
-- Security properties:
--   P1 (no secret in form fields): payload_enc is AES-GCM under MasterKey.
--   P2 (session binding): session_id + actor_id must match on apply.
--   P3 (single-use): row is deleted on apply; reuse → NotFound.
--   P4 (expiry): expires_at enforced on apply; purge_expired removes stale rows.
--   P6 (non-secret audit): summary column is human-readable, no raw secrets.
CREATE TABLE pending_settings_change (
    id          TEXT    PRIMARY KEY,   -- UUID (PendingChangeId)
    session_id  TEXT    NOT NULL,      -- bound to the creating session
    actor_id    TEXT    NOT NULL,      -- bound to the creating admin
    intent      TEXT    NOT NULL,      -- e.g. "smtp_password_update"
    payload_enc BLOB    NOT NULL,      -- MasterKey-AES-GCM encrypted JSON
    summary     TEXT    NOT NULL,      -- non-secret human-readable text
    csrf_token  TEXT    NOT NULL,      -- CSRF token valid for the confirm POST
    expires_at  TEXT    NOT NULL,      -- ISO-8601 UTC; 5-minute TTL
    created_at  TEXT    NOT NULL       -- ISO-8601 UTC
);

CREATE INDEX idx_pending_settings_change_expires
    ON pending_settings_change (expires_at);
