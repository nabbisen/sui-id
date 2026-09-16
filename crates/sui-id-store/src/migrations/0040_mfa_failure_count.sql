-- RFC 102 A8 / L07: consecutive wrong second factors at sign-in, counted on
-- the user rather than on the pending-MFA row. A per-row count is bypassed
-- by minting a new pending row with each correct password. L02 resets it.
ALTER TABLE users ADD COLUMN mfa_failure_count INTEGER NOT NULL DEFAULT 0;
