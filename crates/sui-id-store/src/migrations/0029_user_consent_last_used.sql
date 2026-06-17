-- Migration 0029 — RFC 072: track when each consent grant was last used (v0.60.0)
--
-- NULL means "never observed since this column was added"; the UI
-- renders the original granted_at date in that case so the display
-- is always informative.
ALTER TABLE user_consent ADD COLUMN last_used_at TIMESTAMP;
