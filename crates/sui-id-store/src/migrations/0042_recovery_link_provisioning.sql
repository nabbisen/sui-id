-- RFC 115 D11: a link issued for an account that has just been created is
-- *provisioning*, and is counted against its own, larger hourly ceiling
-- instead of RFC 103 D8's five.
--
-- `provisioning` is set by U37 inside its own transaction, from the target's
-- state at that moment: a local, non-administrator account that has never held
-- a credential, never signed in, and has never had a token of any kind. The
-- column only records the outcome so the two throttles can be counted from the
-- database, the same way D8 is. Every existing row is an ordinary link (0).
--
-- It is added with `ALTER TABLE ... ADD COLUMN`, as 0041 was: `issued_via`
-- cannot gain a value without rebuilding the table.

ALTER TABLE password_reset_tokens
    ADD COLUMN provisioning INTEGER NOT NULL DEFAULT 0
        CHECK (provisioning IN (0, 1));
