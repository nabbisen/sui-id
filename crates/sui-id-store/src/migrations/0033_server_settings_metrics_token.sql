-- RFC 006: add metrics_token_hash to server_settings.
--
-- Stores the bcrypt/Argon2-hashed bearer token used to authenticate
-- Prometheus scrape requests against /metrics.  Nullable: populated
-- on first start when metrics are enabled; NULL means no bearer token
-- has been generated yet.
ALTER TABLE server_settings ADD COLUMN metrics_token_hash TEXT;
