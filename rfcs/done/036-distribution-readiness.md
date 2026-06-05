# RFC 036 — Phase 5: Distribution readiness

**Status.** Proposed  
**Priority.** High — this is the release-gate work before v1.0.  
**Tracks.** ROADMAP Phase 5 "配布".  
**Touches.** `README.md`, `docs/`, `.github/workflows/`, `crates/sui-id/src/main.rs`
(sample config), `Cargo.toml` (package metadata).

## Sub-threads

### A. README corrections

The README still says "JavaScript is reserved for the single `confirm()` prompt on
destructive actions" — RFC 030 replaced all `confirm()` dialogs with server-rendered
confirmation screens. Update the UI Design Notes section.

Also update the Features list to mention:
- MFA (TOTP + WebAuthn passkeys)
- Forgot-password / password-change email notifications
- HIBP breach-password checking
- Session idle timeout / concurrent session cap
- i18n (ja, en, zh)
- Step-up authentication for sensitive operations
- Dangerous-operation confirmation screens (RFC 030)
- Admin operator prompts (RFC 031)
- Audit log integrity via hash-chain (RFC 033)

### B. `docs/` mdbook skeleton

Add a `docs/src/` tree and `docs/book.toml` so the docs can be rendered with
`mdbook build`. The existing `docs/*.md` files are moved/linked as the
source chapters.

Chapter structure per development instruction:
```
docs/src/
├── SUMMARY.md
├── getting-started/
│   ├── overview.md
│   ├── quick-start.md
│   └── faq.md
├── guides/
│   ├── deployment.md        ← existing docs/deployment.md
│   ├── operators.md         ← existing docs/operators.md
│   └── upgrade.md           (new)
├── reference/
│   ├── configuration.md
│   ├── oidc-api.md          ← existing docs/integrators.md
│   └── audit-events.md      (new — lists all event names + labels)
└── contributing/
    ├── architecture.md
    ├── local-dev.md
    └── translators.md
```

### C. `--print-sample-config` command

The README Quick Start shows `sui-id --print-sample-config` but this CLI
flag may not be implemented. Verify and implement if missing.

### D. GitHub CI workflow update

`.github/workflows/ci.yml` — verify it runs `cargo test --workspace` and
`cargo check --workspace`. Update action versions if stale.

### E. `docs/reference/audit-events.md`

Auto-generate from the `audit_event_*` i18n keys: a reference table of all
audit event names, their human-readable labels (Ja/En), and the condition
under which they are emitted. This is the document operators use to write
log-filter queries.

## Version

Patch bumps as sub-threads land. No schema changes, no API changes.
