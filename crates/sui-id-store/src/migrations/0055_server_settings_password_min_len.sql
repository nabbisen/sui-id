-- RFC 151: `server_settings` gains `password_min_len`, the minimum
-- character count a *new* password must meet. Defaults to 12, so an
-- upgrade changes nothing. The `CHECK` is the store's own half of the
-- stricter-only floor (RFC 151 Decision 1): a direct `UPDATE` to a
-- value below 12 or above 128 fails here even if every other check
-- were bypassed. `commands::PASSWORD_MIN_LEN_FLOOR`/`_CEILING` name
-- the same two numbers for the command and HTTP layers; the migration
-- cannot itself derive from a Rust constant, so a store test
-- (`password_min_len_check_matches_the_named_floor_and_ceiling`) pins
-- these two literals against them.
ALTER TABLE server_settings ADD COLUMN password_min_len INTEGER NOT NULL
    DEFAULT 12 CHECK (password_min_len BETWEEN 12 AND 128);
