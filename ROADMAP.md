# Roadmap

This file is a loose sketch of direction — nothing here is a promise.
Completed work is tracked in [CHANGELOG.md](CHANGELOG.md) and the
[`rfcs/done/`](rfcs/done/) directory.

---

## Active proposals (proposed RFCs)

| RFC | Title | Priority | Notes |
|---|---|---|---|
| [RFC 004](rfcs/proposed/004-federation.md) | OIDC/SAML federation (upstream IdP) | Low | Identity provider chaining |
| [RFC 005](rfcs/proposed/005-pluggable-user-backends.md) | Pluggable user backends | Low | LDAP/AD directory integration |
| [RFC 006](rfcs/proposed/006-metrics.md) | Metrics and observability | Low | Prometheus / OpenTelemetry |
| [RFC 008](rfcs/proposed/008-third-party-posture.md) | Third-party posture / consent screen | Low-Medium | Explicit consent for external RPs |
| [RFC 009](rfcs/proposed/009-sql-backends.md) | Alternative SQL backends | Low | PostgreSQL / MySQL support |
| [RFC 017](rfcs/proposed/017-ui-ux-design-contracts.md) | UI/UX design contracts | Medium | Cross-cutting admin UI contract; see [docs/ui-ux-contracts.md](docs/ui-ux-contracts.md) |
| [RFC 023](rfcs/proposed/023-visual-design-system.md) | Visual design system (CSS tokens) | Medium | CSS variable tokens, component primitives, dark mode |
| [RFC 025](rfcs/proposed/025-multi-tenant-expansion.md) | Multi-tenant expansion | Low | Per-tenant namespaces (post-1.0) |

---

## Near-term (next 1–2 releases)

**RFC 023 — Visual design system** is the next planned work. It turns the
colour palette and component sketches from the UI/UX deliverables into
shipped CSS that every `sui-id-web` component inherits. Without it, new
admin-domain screens (RFC 002, RFC 008) each re-derive their own visual
choices.

**RFC 002 — i18n expansion** follows RFC 023 and RFC 017. The typed
`Strings` framework is already in place; this RFC fills in the missing
admin-panel translations and enforces the per-screen completeness rule
from RFC 017 § 4.

---

## Completed (recent)

| Version | What shipped |
|---|---|
| v0.38.0 | e2e coverage (RFC 030/033/035), audit-events doc, settings i18n section headers |
| v0.37.0 | RFC 029 pass 2 (dynamic locale), RFC 035 (user detail), RFC 036 (docs/Phase 5) |
| v0.36.0 | RFC 030 (dangerous ops confirm), RFC 031 (dashboard prompts), RFC 033 (audit), RFC 034 (passkey+empty) |
| v0.35.0 | RFC 032 (dev mode banner), RFC 029 first pass (admin i18n) |
| v0.34.0 | RFC 002 (i18n: zh locale, Formatters, audit labels, dir=, per-recipient locale) |
| v0.33.0 | RFC 001 (email outbox + retry worker) |
| v0.32.0 | RFC 017 (UI/UX contracts), RFC 023 (visual design system), RFC 024 (doc consolidation) |
| v0.31.0 | RFC 014 (hot-path caches), RFC 028 (copy buttons) |
| v0.30.0 | RFC 013 (async DB layer — full implementation + test fixes) |
| v0.29.13 | RFC 026 (admin logout), RFC 027 (client scope UX), dup-username bug fix |
| v0.29.12 | RFC 013 async DB layer initial |
| v0.29.10–11 | RFC 021/022 (schema invariants, boolean CHECKs, migration safety) |

Full history: [CHANGELOG.md](CHANGELOG.md)

---

## Constraints and non-goals (pre-1.0)

- **Single realm.** All users share one namespace. Per-tenant isolation is
  RFC 025, post-1.0. See [docs/operators.md](docs/operators.md) §
  "User–client relationship".
- **SQLite only.** Alternative backends are RFC 009, low priority. The
  current SQLite implementation is production-grade for small deployments.
- **No user-facing theming API.** CSS tokens are for the maintainer, not
  operators.
- **No plugin system.** RFC 005 sketches one; it is not scheduled.
