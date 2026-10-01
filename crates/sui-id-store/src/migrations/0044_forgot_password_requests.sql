-- Migration 0044 — forgot_password_requests (RFC 124 D1)
--
-- The `/forgot-password` request path performs no classification: it
-- records that a recovery was requested for the submitted address,
-- unconditionally and without looking anything up, then responds. A
-- background worker claims pending rows and does everything that used to
-- run inline — the source and credential checks, the outstanding-token
-- throttle, the token mint, the mail dispatch.
--
-- A row recorded here and not yet processed when the process restarts is
-- not lost: it is still 'pending' (or reset to 'pending' from 'processing'
-- by the worker's startup sweep) when the worker starts again. This is the
-- property a spawned, unrecorded background task would not have had.
--
-- `email` is stored in plaintext. This does NOT match `users.email`'s
-- precedent: that column holds the address of someone who registered;
-- this one holds any address anyone submitted, including an attacker's
-- probe list for addresses that match no account — exactly what
-- `events.rs`'s `PasswordResetRequested` deliberately does not record,
-- so a probe can't derive matched-vs-unmatched from the actor column.
-- The architect's decision (RFC 124 stage 2 review, 2026-10-01): keep the
-- column, because the retention is transient by design, not permanent —
-- a row is deleted the moment the worker finishes processing it, and
-- there is no pruning mechanism beyond that. If the worker stops running,
-- submitted addresses accumulate here in plaintext; see
-- `docs/src/guides/operators.md`'s "Operational model" for the
-- operator-facing statement of that fact (RFC 127: this is not allowed to
-- live only in this comment).

CREATE TABLE forgot_password_requests (
    id              TEXT    PRIMARY KEY,
    state           TEXT    NOT NULL
                    CHECK (state IN ('pending', 'processing')),
    email           TEXT    NOT NULL,
    requester_ip    TEXT,
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL
);

-- Only pending rows need the worker's scheduler to find them.
CREATE INDEX idx_forgot_password_requests_pending
    ON forgot_password_requests (created_at)
    WHERE state = 'pending';
