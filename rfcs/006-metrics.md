# RFC 006 — Prometheus metrics endpoint

**Status.** Exploratory
**Tracks.** ROADMAP / Longer term — "Metrics".
**Touches.** new module `sui-id-core::metrics`, one new HTTP
route (`/metrics`), `sui-id-store::repos::audit` lightly (counters
hooked off existing event emission), `Cargo.toml` (`prometheus`
crate or `metrics`+`metrics-exporter-prometheus`).

## Summary

Expose a Prometheus-format `/metrics` endpoint behind admin
authentication. Counters and histograms cover the operationally
interesting events: sign-in attempts (success/failure/lockout),
token issuance and revocation, MFA enrolment activity,
forgot-password requests, audit-write rate, outbox queue depth
(if RFC 001 has shipped), HTTP request latency by route bucket.

This is an "operate it, don't add new behaviour" feature. No
schema change, no new attack surface beyond the auth-gated
endpoint itself.

## Design

### Library choice

Two reasonable picks:

- `prometheus = "0.13"` — direct, zero-magic. We register
  counters and gauges manually, increment them from the call
  sites. ~5 lines of dependency.
- `metrics` + `metrics-exporter-prometheus` — facade-and-impl
  separation, idiomatic if we want pluggable backends later.

Recommend the first. sui-id is a single binary; the facade
buys nothing here and adds two crates of indirection.

### Metrics catalog

Counters (monotonic):

- `sui_id_signin_attempts_total{result="success|wrong_password|locked|mfa_failed|disabled"}`
- `sui_id_signin_via_passkey_total`
- `sui_id_token_issued_total{kind="access|refresh|id"}`
- `sui_id_token_revoked_total{reason="logout|admin|theft_detected|expired_gc"}`
- `sui_id_mfa_enrolled_total{kind="totp|webauthn"}`
- `sui_id_mfa_recovery_consumed_total`
- `sui_id_forgot_password_requested_total`
- `sui_id_audit_appended_total`
- `sui_id_email_outbox_enqueued_total` (if RFC 001 shipped)
- `sui_id_email_outbox_failed_total{reason="transport|template|permanent"}`

Gauges (point-in-time):

- `sui_id_active_sessions` — count of sessions with
  `revoked_at IS NULL` and `last_used_at` within idle window.
- `sui_id_outbox_queue_depth{state="queued|sending|failed"}`
  (if RFC 001 shipped).
- `sui_id_signing_keys_active`, `sui_id_signing_keys_retired`.

Histograms:

- `sui_id_http_request_duration_seconds{route, status_class}`
  — bucketed at 5ms, 25ms, 100ms, 500ms, 2s, 10s.
- `sui_id_argon2_verify_duration_seconds` — useful for
  detecting Argon2 parameter drift and for verifying the
  timing-equivalence dummy hash dwell time.

Excluded by design:

- Per-user metrics. Don't expose per-user series; cardinality
  explosion + privacy.
- Per-client metrics with Client ID as a label. Same reason —
  Client ID is a UUID and produces unbounded cardinality.

### Endpoint

`GET /metrics` mounted on the same listener. Authenticated via
either:

- Admin session cookie, OR
- A new `metrics_token` config value, presented as
  `Authorization: Bearer <token>`. This is the path Prometheus
  scrape configs use. The token is a 32-byte random value,
  generated at first start, stored hashed in
  `server_settings.metrics_token_hash`. Compared in constant
  time. Operators rotate via a `sui-id admin rotate-metrics-token`
  CLI subcommand.

If neither auth method is present, return 401. The body is
Prometheus text format; nothing fancy.

### Configuration

```toml
[server]
metrics_enabled = true            # default false
metrics_listen_addr = ""          # blank = same listener as main app
                                   # set to "127.0.0.1:9090" to split
```

Splitting the listener (private metrics port) is the strongly
recommended deployment posture. We keep it optional because
single-listener also works for trivial deployments and matches
sui-id's "minimum operating ceremony" stance.

## Tests

- Counter increments: hit `/admin/login` with wrong password,
  scrape `/metrics`, assert
  `sui_id_signin_attempts_total{result="wrong_password"}` went up.
- Auth gating: scrape `/metrics` without credentials → 401.
- Bearer token path: stand up a `metrics_token`, scrape with
  the bearer header, succeed.
- Disabled by default: with `metrics_enabled = false`, route
  returns 404. (Active route registration must be conditional;
  no exposure of "metrics endpoint exists but is empty".)

## Security considerations

- **Information disclosure.** The metric values themselves are
  operationally interesting and could help an attacker time
  reconnaissance (sign-in failure spikes, lockout counters
  rising). Keep `/metrics` access privileged: bearer token +
  preferably a separate listener bound to localhost or a
  management network.
- **No PII.** No user IDs, no client IDs, no IP addresses, no
  email addresses appear as labels or values. The catalog
  above is the entire permissible set; additions go through
  RFC review.
- **Argon2 timing histogram.** Genuinely useful for detecting
  configuration drift (someone halves the parameters
  accidentally). Does not leak password material. Keep it.
- **Token rotation.** `rotate-metrics-token` invalidates the
  old token immediately. Document that Prometheus scrape
  configs need to be updated within the same window;
  there's no grace period for two valid tokens at once.

## Open questions

- Do we expose anything per-locale or per-template for email?
  Recommend no — operators don't need that level, and
  template names are stable strings that aren't sensitive but
  also aren't useful to alert on.
- Histogram buckets above are off-the-shelf. Tune after a
  real deployment generates real distributions.
