-- RFC 008: application-identity and registration columns on clients.
--
-- registered_via   — how the client was registered: 'admin' (default,
--                    existing behaviour) or 'dynamic' (RFC 7591).
-- logo_uri         — URL to the application logo image (validated HTTPS).
-- homepage_uri     — URL to the application's home page.
-- privacy_policy_uri — URL to the privacy policy.
-- tos_uri          — URL to the terms of service.
--
-- All four URI columns are validated at write time: HTTPS only (or
-- http://localhost for development).  Their contents are never fetched
-- (P6).  NULL means not supplied; the consent screen gracefully omits them.
--
-- The consent_policy column already exists (migration 0025).  The default
-- 'none' keeps existing first-party clients working without change.
ALTER TABLE clients ADD COLUMN registered_via TEXT NOT NULL DEFAULT 'admin'
    CHECK (registered_via IN ('admin', 'dynamic'));
ALTER TABLE clients ADD COLUMN logo_uri TEXT;
ALTER TABLE clients ADD COLUMN homepage_uri TEXT;
ALTER TABLE clients ADD COLUMN privacy_policy_uri TEXT;
ALTER TABLE clients ADD COLUMN tos_uri TEXT;
