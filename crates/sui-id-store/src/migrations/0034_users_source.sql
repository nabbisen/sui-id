-- RFC 005: user-source discriminator columns.
--
-- `source`             — origin of the user's identity: 'local' (password in
--                        `credentials` table) or 'ldap' (directory bind;
--                        no `credentials` row ever stored).
-- `external_stable_id` — the opaque identifier from the external source
--                        (DN, objectGUID, entryUUID, …) that never changes
--                        even if the user's display name or email changes.
--                        NULL for `source='local'` users.
--
-- Security note: the partial unique index below enforces that no two users
-- can share the same external identity from the same source, preventing the
-- "objectGUID reuse" scenario where a deleted AD account's GUID is assigned
-- to a different person.
ALTER TABLE users ADD COLUMN source TEXT NOT NULL DEFAULT 'local'
    CHECK (source IN ('local', 'ldap'));
ALTER TABLE users ADD COLUMN external_stable_id TEXT;

CREATE UNIQUE INDEX idx_users_external
    ON users (source, external_stable_id)
    WHERE external_stable_id IS NOT NULL;
