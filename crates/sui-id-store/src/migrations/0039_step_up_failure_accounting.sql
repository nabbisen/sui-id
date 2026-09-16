-- 0039: step-up failure accounting (RFC 102 stage 1).
--
-- `step_up_failure_count` counts consecutive failed step-up
-- re-authentications on one session (command L06). At 5 the session is
-- revoked in the same transaction; a successful step-up resets it to 0.
--
-- `last_step_up_method` records the factor of the most recent successful
-- step-up (`totp` | `webauthn`). It is written from RFC 102 stage 6 (L05)
-- onwards; it is added here so `sessions` is migrated once.
ALTER TABLE sessions ADD COLUMN step_up_failure_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN last_step_up_method TEXT;
