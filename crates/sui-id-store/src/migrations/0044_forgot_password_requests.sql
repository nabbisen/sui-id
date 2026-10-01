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
-- `email` is stored in plaintext, matching `users.email`'s existing
-- precedent — it is an address, not a secret, unlike a reset token.

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
