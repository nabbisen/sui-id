-- RFC 095 M3 stage 4a: persist what dynamic registration already
-- validates and echoes, but has never stored.
--
-- token_endpoint_auth_method, grant_types, response_types — all nullable,
-- no DEFAULT, deliberately. A DEFAULT would make every pre-existing row
-- claim a value nobody recorded, which is exactly the inference RFC 095
-- forbids by name for client provenance and which this decision (stage
-- 4a's own C1) extends to these three fields. NULL means "never
-- recorded"; it must stay distinguishable from "recorded as the
-- default", because stage 4b's enforcement exempts the first and checks
-- the second.
--
-- grant_types and response_types are stored as JSON arrays, matching
-- redirect_uris and post_logout_redirect_uris on this same table.
ALTER TABLE clients ADD COLUMN token_endpoint_auth_method TEXT;
ALTER TABLE clients ADD COLUMN grant_types TEXT;
ALTER TABLE clients ADD COLUMN response_types TEXT;
