-- 0009: hash chain over the audit log.
--
-- Each audit row gets a `hash` derived from the canonical
-- serialisation of (prev_hash || row content). An attacker who
-- compromises the SQLite file and tries to delete or rewrite a
-- row breaks the chain at the next row, which is detectable on
-- next startup or on a deliberate verification pass.
--
-- This is *local* tamper-evidence — it doesn't help against an
-- attacker who controls the binary itself (they can rewrite the
-- chain end-to-end), but it does help against the much more
-- common case of an attacker who has DB access from outside the
-- application: SQL injection, misconfigured backup, file-system
-- access, etc.
--
-- For external timestamping (where the chain head is signed by
-- an outside party), see the operator docs — that's a v0.18+
-- topic and not part of this schema migration.
--
-- Pre-migration rows: `prev_hash` and `hash` default to the
-- empty string. The verifier knows to treat an empty prev_hash
-- on the first row as the chain root, and an empty hash on any
-- row as "this row predates v0.17.0" (a soft "we can't verify
-- back beyond here" signal rather than a tamper detection). New
-- rows from v0.17+ always carry both fields populated.

ALTER TABLE audit_log
    ADD COLUMN prev_hash TEXT NOT NULL DEFAULT '';

ALTER TABLE audit_log
    ADD COLUMN hash TEXT NOT NULL DEFAULT '';
