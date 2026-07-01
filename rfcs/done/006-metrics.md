# RFC 006 — Prometheus Metrics Endpoint

**Status.** Implemented (v0.76.0)
**Priority.** Low. Operability feature; no new behaviour beyond an
auth-gated endpoint. Requires explicit owner direction before scheduling.
**Tracks.** ROADMAP / Longer term — "Metrics".
**Touches.** New module `sui-id-core::metrics`; one new route
(`/metrics`); light hooks off existing event emission in
`sui-id-store::repos::audit` and the auth/token paths; `server_settings`
gains a `metrics_token_hash` column; a CLI subcommand
`admin rotate-metrics-token`; `Cargo.toml` (`prometheus` crate); config
(`metrics_enabled`, optional separate listener).

## Summary

Expose a Prometheus-format `/metrics` endpoint behind admin
authentication. Counters and histograms cover the operationally
interesting events: sign-in attempts (success / failure / lockout), token
issuance and revocation, MFA enrolment, forgot-password requests,
audit-write rate, outbox queue depth (if RFC 001 has shipped), and HTTP
request latency by route bucket.

This is an "operate it, don't add new behaviour" feature. No schema change
to user-facing data, no new attack surface beyond the auth-gated endpoint
itself. The design deliberately excludes per-user and per-client series to
avoid cardinality explosion and privacy leakage.

## Motivation

Operators running sui-id in production have no first-class way to observe
it — sign-in failure spikes, lockout rates, token-issuance volume, and
latency are all invisible without scraping the audit log. A standard
Prometheus endpoint plugs sui-id into the observability stack operators
already run, at low implementation cost and with a bounded, reviewable
metric catalog.

## Background

sui-id already emits structured audit events at every operationally
interesting point (sign-in outcomes, token lifecycle, MFA, password
reset). Metrics counters hook off the same call sites, so the incremental
code is a registration step plus increments at points that already exist.
The design choices that matter are not mechanical — they are *what to
expose* (the catalog) and *how to gate it* (auth + optional separate
listener).

## Target code areas

- **`sui-id-core/src/metrics.rs`** (new) — counter/gauge/histogram
  registry; increment helpers called from the auth, token, MFA, and audit
  paths.
- **`sui-id` router** — `GET /metrics`, mounted conditionally on
  `metrics_enabled`; bearer-token and admin-session auth.
- **`sui-id-store`** — `server_settings.metrics_token_hash` column;
  constant-time comparison helper (reuse existing).
- **CLI** — `sui-id admin rotate-metrics-token`.
- **config** — `[server] metrics_enabled`, `metrics_listen_addr`.

## Security properties / invariants

- **P1 (auth-gated).** `/metrics` requires either an admin session cookie
  or a bearer `metrics_token`. No credential → 401.
- **P2 (constant-time token comparison).** The bearer token is compared
  against `metrics_token_hash` in constant time.
- **P3 (no PII).** No user IDs, client IDs, IP addresses, or email
  addresses appear as labels or values. The published catalog is the
  entire permissible set; additions go through RFC review.
- **P4 (bounded cardinality).** No per-user or per-client (UUID-labelled)
  series. Route labels are a fixed, finite set of route templates, never
  raw paths.
- **P5 (disabled by default).** With `metrics_enabled = false` the route
  is *not registered* (returns 404), so there is no "endpoint exists but
  empty" signal.
- **P6 (token rotation is immediate).** `rotate-metrics-token`
  invalidates the old token at once; there is no two-valid-token grace
  window (documented for scrape-config updates).

## Non-goals

- Per-user or per-client metrics (P4).
- Per-locale or per-email-template metrics (operators do not need that
  granularity; template names are stable but not useful to alert on).
- A pluggable metrics-backend facade (`metrics` + exporter) — sui-id is a
  single binary; the direct `prometheus` crate is sufficient.
- Tracing / distributed spans (out of scope; a separate concern).

## Proposed design

### Library choice

`prometheus = "0.13"` — direct, zero-magic. Counters and gauges are
registered once and incremented at the call sites. The facade-and-impl
split (`metrics` + `metrics-exporter-prometheus`) buys pluggable backends
that a single binary does not need; recommend the direct crate.

### Metric catalog

Counters (monotonic):

- `sui_id_signin_attempts_total{result="success|wrong_password|locked|mfa_failed|disabled"}`
- `sui_id_signin_via_passkey_total`
- `sui_id_token_issued_total{kind="access|refresh|id"}`
- `sui_id_token_revoked_total{reason="logout|admin|theft_detected|expired_gc"}`
- `sui_id_mfa_enrolled_total{kind="totp|webauthn"}`
- `sui_id_mfa_recovery_consumed_total`
- `sui_id_forgot_password_requested_total`
- `sui_id_audit_appended_total`
- `sui_id_email_outbox_enqueued_total` *(if RFC 001 shipped)*
- `sui_id_email_outbox_failed_total{reason="transport|template|permanent"}`

Gauges (point-in-time):

- `sui_id_active_sessions` — sessions with `revoked_at IS NULL` and
  `last_used_at` within the idle window.
- `sui_id_outbox_queue_depth{state="queued|sending|failed"}` *(if RFC 001
  shipped)*.
- `sui_id_signing_keys_active`, `sui_id_signing_keys_retired`.

Histograms:

- `sui_id_http_request_duration_seconds{route, status_class}` — bucketed
  at 5ms, 25ms, 100ms, 500ms, 2s, 10s. `route` is a fixed template label.
- `sui_id_argon2_verify_duration_seconds` — detects Argon2 parameter
  drift and verifies the timing-equivalence dummy-hash dwell time.

### Endpoint and auth

`GET /metrics`, authenticated via an admin session cookie OR a
`metrics_token` presented as `Authorization: Bearer <token>` (the path
Prometheus scrape configs use). The token is a 32-byte random value
generated at first start, stored as `server_settings.metrics_token_hash`,
compared in constant time. Rotation: `sui-id admin rotate-metrics-token`.

### Configuration

```toml
[server]
metrics_enabled = true            # default false
metrics_listen_addr = ""          # blank = same listener; set to
                                   # "127.0.0.1:9090" to split onto a
                                   # private port
```

Splitting the listener onto a private port is the recommended posture;
single-listener stays available for trivial deployments.

## Data model impact

One column: `server_settings.metrics_token_hash` (nullable; populated on
first start when metrics are enabled). No new tables. One migration.

## API impact

One new route, `GET /metrics`, present only when `metrics_enabled = true`.
Prometheus text-format body. No change to any existing endpoint.

## Testing strategy

- Counter increments: a wrong-password `/admin/login` then a scrape shows
  `sui_id_signin_attempts_total{result="wrong_password"}` incremented.
- Auth gating: scrape without credentials → 401 (P1).
- Bearer path: configure a `metrics_token`, scrape with the header,
  succeed.
- Disabled-by-default: with `metrics_enabled = false`, the route returns
  404 (P5) — confirm the route is not registered.
- Constant-time comparison: token check uses the constant-time helper
  (P2).
- No-PII review: a snapshot test asserts the exposed label set matches the
  published catalog exactly (P3/P4).

## Migration strategy

One additive, nullable column. No backfill. Deployments that never enable
metrics never populate it. Fully backwards-compatible.

## Rollout plan

Single increment is feasible, but a clean split is: (1) the registry,
counters, and gauges with the endpoint behind admin-session auth only;
(2) the bearer-token path, `metrics_token_hash` column, and the rotate
CLI; (3) the optional separate-listener config. SQLite-default, single
binary, unchanged at every step. No version designation without owner
direction and soak.

## Risks and mitigations

- *Risk:* information disclosure (failure spikes aid reconnaissance).
  *Mitigation:* P1/P3 — privileged endpoint, no PII, recommend a private
  listener.
- *Risk:* cardinality explosion from per-entity labels. *Mitigation:* P4
  — fixed catalog, no UUID labels; snapshot test enforces it.
- *Risk:* stale scrape config after rotation. *Mitigation:* P6 — document
  the no-grace-window behaviour; rotation is an explicit operator action.

## Acceptance criteria

- `/metrics` returns Prometheus text format only when enabled and
  authenticated; 404 when disabled, 401 when unauthenticated.
- The exposed label set matches the published catalog exactly (enforced
  by test).
- The bearer token is stored hashed and compared in constant time.
- No user/client/IP/email appears anywhere in the output.
- 0 warnings; full suite green; all CI gates hold.

## Open questions

- Per-template email metrics? Recommend **no** — not useful to alert on.
- Histogram bucket boundaries are off-the-shelf; tune after a real
  deployment produces real distributions.
- Should `argon2_verify_duration` be opt-in (it is genuinely useful but
  is a timing signal)? Recommend **on** — it does not leak password
  material and catches parameter drift.
