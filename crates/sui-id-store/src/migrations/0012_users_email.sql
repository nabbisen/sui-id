-- 0012_users_email.sql
--
-- Add an optional `email` column to `users`.
--
-- ## Why
--
-- Two reasons in v0.20.4, with a third in mind for later:
--
-- 1. The setup wizard (画面 2 in the design) asks the operator for
--    an admin email at account creation time. Admin pages already
--    accept the field on user create; we just need somewhere to
--    keep it.
-- 2. The `/admin/users` create form is gaining the same field.
-- 3. (Future, deferred to the email-integration phase) Emails are
--    the recipient address for password-change notifications,
--    forgot-password reset flows, etc — once `wasm-smtp v0.6` lands.
--
-- ## Why nullable
--
-- Existing rows must keep working without backfill. A NULL email
-- means "we don't have one"; OIDC userinfo will simply not include
-- the `email` claim in that case. No code path treats absence of an
-- email as a security issue today, so making the column required
-- would be a needless break.
--
-- ## Why a partial UNIQUE index, not a column-level UNIQUE
--
-- We want at most one user per email *when an email is set*. SQLite
-- treats every NULL as distinct from every other NULL, so a plain
-- `UNIQUE` constraint already permits multiple NULL rows — but
-- being explicit with `WHERE email IS NOT NULL` documents the
-- intent and survives any future SQLite-version changes to NULL
-- handling. The index also speeds up future "look up by email"
-- queries (e.g. forgot-password).

ALTER TABLE users ADD COLUMN email TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email
  ON users (email)
  WHERE email IS NOT NULL;
