-- 0009: hash chain over the audit log.
--
-- Each audit row gets a `hash` derived from the canonical
-- serialisation of (prev_hash || row content). An attacker who
-- compromises the SQLite file and tries to delete or rewrite a
-- row breaks the chain at the next row — but only if verification
-- actually compares row N+1's `prev_hash` against row N's `hash`,
-- and checks `seq` for gaps (RFC 125 D1, D2); until RFC 125 the
-- verifier compared each row's own `hash` to its own `prev_hash`
-- only, which a single `UPDATE` recomputing that one row's own
-- `hash` satisfied. The one row this cannot ever protect, by the
-- nature of a forward hash chain: the current newest row, before
-- anything is appended after it to record its hash.
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
