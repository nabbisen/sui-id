# sui-id — Development Specification

*v3 — reflecting the v0.48.4 codebase. Supersedes the v2 spec
(`sui-id-開発指示書_v2-0_29_1時点.md`, snapshot at v0.29.1). This
document carries forward the original philosophy and direction
unchanged; the technical sections are rewritten against the current
implementation, and several sections are new (RFC lifecycle, CI
invariants, design system, verification phase).*

---

## 0. Project name

**sui-id**  ·  internal identifier: **`sui_id`**

The name *sui* (翠) — the Japanese word for jade — is a deliberate
metaphor. It signals a service that holds no excess ambition, is
quietly dignified, and stays warm and gentle toward the people who
use it. This intent must be reflected throughout: in implementation,
UI, operations, and error handling alike.

---

## 1. Purpose

`sui-id` is a **self-hostable, portable, security-first minimal
IDaaS**. The goal is that developers of any member-facing
application can begin building with authentication and authorisation
in place from day one, without needing to subscribe to a SaaS IdaaS
or stand up a complex user-management stack of their own.

The project simultaneously satisfies:

- Easy to deploy as a single binary
- Local-first
- Safety-first
- Approachable to non-experts
- Yet not over-featured
- Simple, easy to inspect at a glance
- Small file footprint at runtime

---

## 2. Foundational principles

### 2.1 Design philosophy

- **Developer-first** — lower the bar to start a project; design in
  line with widely-understood standards.
- **Local-first** — runnable in a single local environment; avoid
  proliferating dependent services.
- **Safety-oriented** — never store secrets in plaintext; never
  permit unauthorised administrative operations; fail to the safe
  side.
- **Minimal** — only the necessary features; don't over-build.
- **Accessible by default and by design** — usable by beginners;
  clarity is built in from the design stage, not bolted on later.
- **Unix philosophy** — do one thing well; separate roles; avoid
  excessive integration.

### 2.2 Priorities

Implementation decisions follow this order:

1. **Safety**
2. **Robustness**
3. **Maintainability**
4. **Specification compliance**
5. **Usability**
6. **Aesthetics**

Convenience matters, but never at the cost of safety or robustness.

---

## 3. Out of scope

The following are explicitly outside this specification's
responsibility (future expansion is not denied, but the spec does
not assume it):

- Social login
- External IdP federation
- Distributed / clustered operation
- Large-scale multi-tenant operation
- Advanced organisational hierarchies
- Mail-server infrastructure itself (SMTP **send** is included; an
  outbound mail server is not)
- SMS infrastructure
- Device Flow, Implicit Flow, Hybrid Flow
- Excessively fine-grained permission design
- Advanced analytics dashboards
- Complex workflow engines
- All-in-one IAM-product breadth
- SAML

---

## 4. Glossary

Vocabulary is fixed across implementation and documentation:

- **Administrator** — a privileged user who can configure and
  operate the entire system.
- **Regular user** — an end-user who signs in to a relying application.
- **Client** — an external application; an OIDC / OAuth 2.0
  relying party.
- **Session** — a continuing logged-in state.
- **Authorization Code** — a short-lived single-use code in the
  authorization-code flow.
- **Access Token** — short-lived API token.
- **Refresh Token** — long-lived token used to renew access tokens
  without re-authentication.
- **Signing Key** — Ed25519 key used to sign JWTs.
- **Master Key** — 32-byte symmetric key used for column-level
  encryption.
- **Encrypted storage** — encrypted persistent storage.
- **Initialised** — first admin and initial settings are complete.
- **Uninitialised** — first-run setup has not been done.
- **Logical delete** — mark unusable via a flag.
- **Physical delete** — remove the DB row.
- **Audit log** — a tamper-evident record of administrative actions
  (SHA-256 hash chain).
- **MFA** — multi-factor authentication (TOTP / WebAuthn passkey).
- **Step-up authentication** — re-authenticate immediately before
  a high-impact operation.
- **Locale** — display language identified by a BCP-47 tag.
- **HIBP** — Have I Been Pwned; the breached-password check.
- **Dev mode** — the `--dev` flag startup path (see §11.13).
- **Verification phase** — the v0.48.0-onward operational stage,
  during which actual environments surface latent issues that
  pre-tagging review missed. v1.0 tags are deferred until this
  phase produces sufficient confidence (see §22).

---

## 5. Standards compliance

### 5.1 Posture

- Where the standard defines it, follow the standard.
- Keep custom extensions to a minimum.
- Avoid custom implementations that break interoperability.
- Where the spec leaves room, prefer the standard reading.

### 5.2 Primary specifications adhered to

- OAuth 2.0 authorization-code flow
- PKCE (S256 only)
- OpenID Connect Discovery
- RP-Initiated Logout (`end_session_endpoint`)
- JSON Web Token
- JSON Web Key Set
- JSON Web Signature (common usage)
- RFC 7662 Token Introspection
- RFC 7009 Token Revocation
- OIDC `acr` / `amr` claims
- WebAuthn (Level 2)
- CSRF protection and redirect-URI handling

### 5.3 Adoption rules

- Authorization Code + PKCE is mandatory (`plain` is rejected, S256
  only)
- Discovery and JWKS are provided
- Implicit Flow is not implemented
- Hybrid Flow is not implemented
- Device Flow is not implemented
- `redirect_uri` is matched **exactly** (no prefix matches, no
  wildcards)
- Prefer the minimal OIDC subset needed for federation

---

## 6. Security principles

### 6.1 Assets to protect

- Administrator credentials
- User passwords
- TOTP secrets and passkey public-key records
- Refresh tokens
- Client secrets
- Signing keys
- Master key
- Audit log
- Personally identifying information
- Session state
- SMTP credentials
- Recovery codes

### 6.2 Prohibited

- Plaintext storage of secrets
- Exposing unauthenticated admin APIs
- Exposing admin functionality before initialisation
- Logging secrets
- Unsafe defaults
- Returning excessive internal details on failure
- Lax redirect-URI handling
- Non-constant-time comparisons
- Skipping audit trails for admin actions
- `unsafe` Rust (workspace-wide `unsafe_code = "forbid"`)
- Timing channels in authentication paths (lockout branches, MFA
  branches)

### 6.3 Implementation safeguards

- Secret values are wrapped to suppress accidental logging
  (`secrecy`-style).
- Comparisons that could leak via timing use `subtle::ConstantTimeEq`.
- In production, error responses suppress internal detail and
  return a request ID.
- Panics are suppressed; only catastrophic invariant violations
  abort.
- All logging is `tracing`-based and supports masking.
- Secret values do not appear in `Debug`.
- The principle on failure is **fail to the safe side**, not "hide
  the failure".
- Login-failure, lockout, and MFA-failure response times are
  equalised: dummy Argon2id verification is always run.
- Refresh-token theft detection silently revokes the entire token
  family; the response is indistinguishable from an ordinary
  rotation failure.

---

## 7. Threat model

The detailed model lives in `docs/threat-model.md` (12 scenarios + 8
defensive properties + detailed concerns + known limits). The spec
gives only the summary.

### 7.1 Twelve assumed threats

1. Token leakage (access / refresh)
2. Unauthorised access to the admin UI
3. Admin UI exposure via misconfiguration
4. CSRF
5. XSS
6. Session fixation
7. Open-redirect / redirect-URI abuse
8. Secret leakage through logs
9. Exfiltration of the DB or config file
10. Password brute-force / credential-stuffing
11. Misuse of the email-based password reset
12. Misuse of the passkey-registration path

### 7.2 Eight defensive properties

- Least-privilege defaults
- Strict authn/authz on admin APIs
- Setup mode is first-run only
- Failure paths land on the safe side
- The attack surface stays narrow
- Errors don't over-disclose
- Audit log exists and is tamper-evident
- Lockout / MFA paths are timing-equivalent

### 7.3 Known limits

- Single-instance deployment is assumed; HA configurations are out
  of scope.
- Local-machine physical access is out of scope.
- An attacker who holds the master key can read the entire DB
  (key custody is the operator's responsibility).

---

## 8. Tech stack

### 8.1 Required

- **Rust 2024 Edition**
- **Axum 0.8**
- **Leptos 0.8 (SSR only)**
- **SQLite** via `rusqlite`
- **TOML configuration**
- **Cargo workspace structure**

### 8.2 Principal libraries

| Concern | Library |
|---|---|
| HTTP framework | Axum 0.8 + tower-http 0.6 |
| Admin UI | Leptos 0.8 (server-rendered; minimal hand-written JS) |
| Persistence | SQLite + column-level encryption (XChaCha20-Poly1305 + AAD binding) |
| Crypto | `chacha20poly1305`, `argon2`, `ed25519-dalek`, `webauthn-rs` |
| TOTP | Hand-written (RFC 6238) |
| Mail | `wasm-smtp` (vendored; SMTP/STARTTLS) |
| HIBP | `ureq` + `Add-Padding` k-anonymity |
| Error type | `thiserror` |
| Logging | `tracing` |
| Random | `rand` + `OsRng` |
| Password hash | Argon2id |
| Serialisation | `serde`, `serde_json` |
| Testing | unit + integration kept separate |

### 8.3 Constraints

- **`mod.rs` is not used.** Use Rust 2018+ "umbrella `foo.rs` +
  sibling `foo/` directory" style throughout. Phase F (RFCs
  065–068) made this hard policy; it must hold.
- Dependencies are deliberately kept thin. Convenient-but-heavy
  crates are reviewed carefully before adoption.
- Crate boundaries follow responsibility, not size.
- **Per-file line-count policy**:
  - Effective lines of code (comments excluded) ≥ **500** lines:
    a file is a **split candidate**.
  - ≥ **300** lines: consider whether a split would help.
  - Test modules inside `src/` are separated as
    `parent/tests.rs`, with further splits under `parent/tests/`.
  - Integration tests keep one binary per entry point: each entry
    is `tests/<entry>/main.rs` declared in `[[test]]` `path =`,
    and theme files are pulled in with `mod` declarations.
  - Genuinely cohesive files that resist meaningful splits are
    permitted to exceed the recommendation; readability of the
    whole takes precedence over the line-count metric alone.
- `unsafe_code = "forbid"` applies workspace-wide.
- License is Apache-2.0.

---

## 9. Project structure

```text
.
├── Cargo.toml                    # virtual workspace
├── crates/
│   ├── sui-id/                   # binary + router + handlers + CLI
│   ├── sui-id-core/              # use-case layer (auth, OIDC, state)
│   ├── sui-id-store/             # SQLite + migrations + crypto
│   ├── sui-id-web/               # Leptos SSR + design system
│   ├── sui-id-i18n/              # three locale tables
│   └── sui-id-shared/            # DTOs + typed IDs + AuthMethod
├── docs/
│   ├── src/                      # mdbook-compatible content
│   │   ├── SUMMARY.md
│   │   ├── introduction.md
│   │   ├── getting-started/{quick-start,overview,faq}.md
│   │   ├── guides/{operators,deployment,upgrade,dangerous-operations}.md
│   │   ├── reference/{configuration,oidc-api,audit-events}.md
│   │   └── contributing/{architecture,local-dev,state-contract,translators}.md
│   ├── ui-ux-contracts.md        # cross-cutting UI/UX rulebook
│   ├── threat-model.md
│   └── assets/logo.{png,svg}
├── rfcs/
│   ├── README.md
│   ├── 000-rfc-lifecycle-policy.md
│   ├── done/                      # 60+ implemented RFCs
│   └── proposed/                  # open candidates
├── examples/dev-seed.toml
├── .github/                       # CI / SECURITY / templates
├── .vscode/
├── README.md / LICENSE / NOTICE
├── CHANGELOG.md / ROADMAP.md
└── sui-id.example.toml
```

### 9.1 Crate responsibilities

| Crate | Responsibility |
|---|---|
| **`sui-id`** | Executable. `main.rs`, axum bootstrap, router, asset embedding, config load, CLI subcommands (`backup`, `restore`, `verify-backup`, `admin unlock-user`, `admin rotate-key`), `--dev` mode (`src/dev_mode.rs`), HTTP handlers (`src/handlers/`). Static JS lives in `crates/sui-id/static/`. |
| **`sui-id-core`** | Use-case layer free of handler/HTTP concerns. Authn/authz, OIDC code/token/discovery/JWKS/introspection/revocation, password hashing, JWT signing, MFA (TOTP + WebAuthn + recovery codes), session lifecycle (idle timeout, concurrent-session cap), lockout, step-up, mail dispatch, HIBP client, master-key rotation, domain error types. |
| **`sui-id-store`** | SQLite persistence, migrations, column-level encryption, repository implementations, audit-log persistence (SHA-256 hash chain). |
| **`sui-id-web`** | Leptos SSR. Admin / setup / settings / self-service UIs, design tokens (`tokens.rs`), component CSS (`components.rs`), layout shells (`layout.rs`), per-screen render functions (`pages/`). |
| **`sui-id-i18n`** | `Locale` enum + `Strings` struct, per-locale files under `locale/` (`en.rs`, `ja.rs`, `zh_hans.rs`, `zh_hant.rs` stub), Accept-Language negotiation. |
| **`sui-id-shared`** | Cross-crate DTOs, typed UUID IDs (`UserId`, `ClientId`, `SessionId`, …), `AuthMethod` enum. |

---

## 10. Minimal runtime footprint

### 10.1 Files

- The executable (`sui-id`)
- The configuration file (`sui-id.toml`)
- The encrypted store (`sui-id.db` etc.)
- The master key (resolved via env var or key file path)
- Minimal log output

### 10.2 Principles

- Don't add files unnecessarily.
- Don't scatter configuration.
- Keep "what is needed" obvious.
- An operator should be able to look at the file set and
  understand it.

### 10.3 Key management

- The master key is **never** kept in the config file in plaintext.
- It may be generated at first-run setup.
- It may be injected via environment variable or an explicit path.
- Loss of the master key is unrecoverable; this is stated
  explicitly to operators.
- Backup guidance covers both the DB and the key.
- A CLI rotates the master key (`sui-id admin rotate-key`): offline
  flow, all encrypted columns re-sealed under the new key, old key
  file renamed to `<original>.bak.<timestamp>`. SQLite transaction
  rolls back partial states on failure.

---

## 11. Functional requirements

### 11.1 Authentication / authorisation

- Acts as an OIDC Provider.
- Authorization Code + PKCE (S256).
- Discovery at `/.well-known/openid-configuration`.
- JWKS at `/.well-known/jwks.json` (multiple key generations may
  be published simultaneously).
- RP-Initiated Logout (`end_session_endpoint`).
- Access Tokens are JWTs signed with Ed25519.
- Refresh Tokens are managed with rotation and family-wide
  revocation on theft detection.
- Token Introspection (RFC 7662).
- Token Revocation (RFC 7009).
- `acr` and `amr` claims are included in ID Tokens.
- The ID Token also carries `email` / `email_verified` when the
  granted scope includes `email` (OIDC Core §5.1, added v0.48.3).
- Refresh exchanges preserve the originating authentication
  methods.
- Per-client settings: `redirect_uris`, `post_logout_redirect_uris`,
  allowed scopes, etc.

### 11.2 User management

- Create, list, disable / re-enable, delete users.
- Password reset (admin-initiated + email-initiated).
- Force-logout (single user or all).
- Logical delete is used where appropriate.
- HIBP check on password set (off / warn / block).

### 11.3 MFA

- TOTP (RFC 6238).
- WebAuthn passkeys, multiple per user, each with a nickname.
- Eight recovery codes, shown once on initial setup, regeneratable.
- Admin-initiated MFA reset.
- All MFA paths are timing-equivalent (failure branches run dummy
  verification).

### 11.4 Step-up authentication

- Re-authenticate immediately before high-impact actions.
- TOTP code **or** passkey.
- Valid for 5 minutes.
- Only irreversible / system-wide-impact actions trigger it
  (password change is deliberately exempt).
- WebAuthn pending rows for step-up are tagged `kind = 'step_up'`
  to prevent cross-context misuse.

### 11.5 Self-service security (`/me/security/*`)

The self-service surface is the **canonical** location for
user-owned security operations (RFC 055, v0.44.0). Tabs are
deep-linkable (RFC 040, v0.43.0):

- `/me/security/overview`
- `/me/security/mfa`
- `/me/security/sessions`
- `/me/security/passkeys`
- `/me/security/language`
- `/me/security/password` (a tab inside the same shell)

Provides: password change, MFA enable / disable / recovery-code
regenerate, passkey CRUD, active-session listing + revoke (one
or all-other), language preference, recent security-related audit
history.

`/admin/profile` is preserved as a legacy 308-redirect to
`/me/security/overview`.

### 11.6 OIDC-client management

- Issue Client IDs (UUID).
- Issue Client Secrets for confidential clients.
- Configure `redirect_uris` (exact match).
- Configure `post_logout_redirect_uris`.
- Restrict allowed scopes.
- Register PKCE-only public clients.
- Disable / delete clients.
- View client details.

### 11.7 Admin panel

- Service status.
- Dashboard sparkline + recent events.
- User management.
- Client management.
- Signing-key management (generation rollover, retirement).
- Settings inspection.
- Safe settings editing organised into six tabs:
  `basic / authentication / email / security / logs / other`.
- Dangerous-operation confirmation screens (RFC 030).
- Audit log viewer.

### 11.8 Mail

- SMTP settings configurable in admin UI; credentials are
  encrypted at rest.
- `/forgot-password`: token is 32-byte CSPRNG → URL-safe base64,
  stored hashed, 30-minute TTL, single-use, max 3 active per user.
- `/forgot-password` always returns 200 with a neutral page
  (no user enumeration).
- Password-change notifications dispatched automatically.
- When SMTP is unconfigured, `/forgot-password` returns 404.
- Mail dispatch through a persistent **outbox** (RFC 001):
  enqueue inline, deliver from a background worker, audit on
  failure, continue regardless.

### 11.9 Session limits

- Idle-session timeout (configurable; `0` disables).
- Concurrent-session cap (configurable; `0` disables) — FIFO
  expiry of the oldest session.
- `last_used_at` is updated with 60-second throttling.
- Expiry attempts are best-effort; failures do not block the user
  experience.

### 11.10 Internationalisation

Languages: **Japanese (default)** and **English**. Simplified Chinese
(`zh-Hans`) is compiled and maintained in `locale/zh_hans.rs` but is
not yet included in `Locale::ALL` — it requires a full copy review
before being surfaced as a server-default option. A Traditional
Chinese (`zh-Hant`) stub exists in `locale/zh_hant.rs`; it delegates
to `zh-Hans` until a contributor supplies reviewed translations.
See `docs/src/contributing/translators.md` for how to add or promote
a locale.

Resolution chain (top to bottom):

1. `users.preferred_lang` (when authenticated).
2. Cookie `sui_id_lang`.
3. `Accept-Language` header.
4. `server_settings.default_lang` (admin-set).
5. Hard-coded `Locale::Ja`.

The setup wizard adds an **explicit language picker** at the top
of its welcome screen (v0.48.2) so an operator whose browser
sends `Accept-Language: en` can still install a Japanese-default
deployment, and vice versa.

`<html lang="…">` reflects the resolved locale. Translation
completeness is enforced by the Rust type system through the
exhaustive `Locale::strings()` match. Adding a new language is
a `Locale` variant + a `static STRINGS_<LANG>` constant + a match
arm.

### 11.11 Auditing

- Every administrative action is recorded with actor, time, action,
  outcome.
- SHA-256 hash chain (previous-hash + length-prefixed canonical
  byte sequence) detects tampering.
- Event names are stable, dot-delimited lower-case strings, e.g.
  `auth.mfa.failure`, `admin.master_key.rotated`.
- Audit rows are not deleted lightly; physical delete is cautious.

### 11.12 First-time setup

- On first run, if uninitialised, the setup wizard launches
  (welcome → admin → language → HIBP → done).
- A one-time setup token is generated and **printed to stderr as a
  complete clickable URL** (v0.48.4):
  `Open the following URL: http://host:port/setup?token=…`. The
  operator does not copy-paste the raw token into a text field;
  the token rides as a URL parameter through to the admin form,
  where it is a hidden input.
- The welcome page lets the operator pick the wizard's language
  explicitly.
- The wizard creates the first admin, generates initial keys,
  accepts basic configuration choices, and configures HIBP mode
  (off / warn / block).
- Once complete, the system transitions to normal operation; the
  wizard endpoint is closed.

### 11.13 Dev mode

- `--dev` skips the setup wizard entirely.
- Uses an in-memory SQLite database with an ephemeral master key.
- Hard-coded seed: admin / alice / bob with 12-character
  human-recognisable passwords; one test OIDC client.
- Optional hybrid seed: a TOML at `--dev-seed PATH` overrides the
  hard-coded defaults; CLI flags (e.g. `--dev-admin-password`)
  override the TOML.
- Default bind is `127.0.0.1`; non-loopback binds require typing
  `yes` on stdin to confirm.
- Startup banner prints "DEV MODE" plus all plaintext credentials
  to stderr.
- **Cryptographic invariants stay identical to production**: PKCE
  S256 only, Argon2id parameters, AAD binding, exact-match
  `redirect_uri`, ≥12-char password policy, `unsafe_code = forbid`.
- **Operational relaxations are visible**: `cookie_secure = false`,
  `hibp_mode = off`, lockout disabled. The browser banner makes
  dev-mode obvious to anyone glancing at the page (RFC 032).

### 11.14 Backup / restore

- `sui-id backup --to PATH`
- `sui-id restore --from PATH`
- `sui-id verify-backup --from PATH`
- `--encrypt` for optional passphrase-based encryption.
- `--force` for explicit overwrite of existing destination files.

---

## 12. Setup-wizard policy

### 12.1 State-driven entry

- Uninitialised → wizard.
- Initialised → admin UI or login.

### 12.2 Safety requirements

- Normal admin functionality is never exposed before initialisation.
- After completion, the wizard cannot be re-run.
- Re-initialisation requires an explicit maintenance procedure.
- The wizard is reachable only through a safe path.
- The master key is **never** handled in the wizard UI; it is
  resolved before HTTP starts.

### 12.3 Initial settings collected

- Administrator account (≥12-character password)
- Display language
- Logging policy
- Master-key generation or injection (out-of-band)
- HIBP mode
- Basic operational settings

---

## 13. Data model

### 13.1 Principal entities

| Entity | Notes |
|---|---|
| **User** | `username` unique; `email` nullable with a partial-unique constraint |
| **Credential** | Password hash and TOTP secret in separate tables |
| **Client** | OIDC client |
| **AuthorizationCode** | Short-lived; deleted on consume |
| **Session** | Carries `last_used_at` |
| **RefreshToken** | Family ID + parent-token pointer |
| **SigningKey** | Generations: `active` / `retired` (both published in JWKS) |
| **Consent** | Reserved for future expansion |
| **WebauthnCredential** | Multi-passkey per user |
| **WebauthnPending** | `kind` column discriminates step-up / register / login |
| **AuditLog** | SHA-256 hash chain |
| **PasswordResetToken** | Hashed token, TTL, single-use |
| **SmtpConfig** | Singleton; credentials encrypted |
| **ServerSettings** | Singleton: `default_lang`, `hibp_mode`, `idle_session_timeout_secs`, `max_concurrent_sessions` |

### 13.2 Treatment principles

- User identity is separated from credentials.
- Tokens carry expiry and revocation state.
- The audit log is tamper-resistant.
- Signing keys are generation-managed; both active and retired
  generations are published in JWKS.
- Encryption is **column-level**, not table-level: `secret`,
  `password_hash`, `recovery_code_hash`, `webauthn_credential`,
  `smtp_password_enc`, etc.
- Every encrypted column carries a column-specific AAD to prevent
  cross-column ciphertext substitution.
- Consent retention is reserved for future expansion.

---

## 14. Data retention and deletion

### 14.1 Deletion model

- Users and clients may be logically deleted (`is_deleted` flag).
- Sessions and refresh tokens distinguish *revocation* from
  *physical deletion*.
- Audit retention is governed separately.
- Physical deletion is cautious.

### 14.2 Things that get revoked

- Sessions on logout.
- Sessions targeted by force-logout.
- Revoked / used refresh tokens.
- Authorisations attached to disabled clients.
- Entire refresh-token family on theft detection.

### 14.3 Cautions

- The audit log is not deleted casually.
- Records needed for incident review are preserved.
- "What is deleted when" is documented in the operator manual.

---

## 15. Logging and audit

### 15.1 Logging

- All logging is via `tracing`.
- Sensitive values are masked.
- Production suppresses detailed internal information.
- Each request carries an ID; users receive it on errors.

### 15.2 Audit

The audit log captures: who, when, what, outcome. Coverage:

- Admin actions on users (create / disable / delete / MFA reset).
- Admin actions on clients (create / update / delete / secret
  rotate).
- Signing-key operations.
- MFA enable / disable / failure.
- Master-key rotation (`admin.master_key.rotated`).
- Refresh-token theft detection (`auth.refresh.family_revoked`).

A SHA-256 hash chain (each row hashes the canonical bytes of the
prior hash + the current event) provides tamper evidence.

---

## 16. Error handling

### 16.1 Posture

- Domain errors are defined with `thiserror`.
- API responses return opaque error codes.
- Critical errors fail to the safe side.
- Panics are suppressed.

### 16.2 Display

- Development surfaces detail.
- Production suppresses detail.
- User-facing copy is short and plain.
- Internal causes go to the log only.

### 16.3 HTML representation: redirect, not 401-page

The HTML representation of `CoreError::Unauthenticated` redirects
to `/admin/login` rather than rendering a 401 page (v0.48.1).
The 401 page exists for genuine error conditions (malformed
cookie, server failure), not for "you need to sign in".

Error pages are reachable; their "Back home" link is
**context-aware**: 401 → `/admin/login`, everything else → `/`.

### 16.4 Timing equivalence

- Login / lockout / MFA paths take constant-equivalent time.
- Dummy Argon2id verification is always run on the failure branch.
- Refresh-token theft detection is indistinguishable from an
  ordinary rotation failure.

---

## 17. UI / UX

### 17.1 Admin panel

- Simple, easy to follow.
- Avoid decorative excess.
- Wording is approachable to first-time operators.
- Dangerous actions are confirmed.
- Major operations complete with few page transitions.

### 17.2 Accessibility (Accessible by Default and by Design)

- Screen-reader aware.
- Labels are explicit.
- Information is not conveyed by colour alone.
- Keyboard-only operation is supported.
- `:focus-visible` provides a 2-px focus ring.
- Error copy is short and specific.

### 17.3 Settings editing

- Arbitrary dangerous TOML editing is not permitted.
- Safe settings are exposed as forms.
- Dangerous settings are isolated under an "advanced" section.
- Confirmation precedes a change.

### 17.4 Design system

The design system is concrete and bounded; it lives in
`crates/sui-id-web/`.

**Tokens** (`tokens.rs`, ~300 LOC, RFC 049 vocabulary freeze):

- Spacing: `--space-1` .. `--space-6` (8 / 12 / 16 / 24 / 32 / 48 px)
- Foreground: `--fg-default`, `--fg-muted`, `--fg-on-accent`
- Surface: `--surface-default`, `--surface-subtle`, `--surface-elevated`
- Accent: `--accent-default`, `--accent-subtle`
- Semantic palette: for each of `danger / warning / success / info`,
  the triple `--{name}-default` / `--{name}-subtle` /
  `--fg-on-{name}` (RFC 061; CI gate `semantic-palette-parity`)
- Border / radius / state: `--border-muted`, `--border-strong`,
  `--border-width-default`, `--radius-sm`, `--radius-md`,
  `--state-hover`, `--state-active`
- Typography: `--font-size-caption` .. `--font-size-h1`,
  `--font-weight-medium`, `--font-family-system` (system stack only;
  zero web-font assets)
- Layout: `--content-max-width` (64rem), `--content-narrow-width`
  (28rem)

**Theme**: `[data-theme]` on `<html>`. `theme-init.js` (loaded as
a CSP-safe external script) applies the operator's choice from
`localStorage` before first paint; absence falls back to
`prefers-color-scheme`. `::selection` uses `--accent-default` plus
`--fg-on-accent` for unambiguous visibility (v0.48.2 fix).

**Components** (`components.rs`, ~1000 LOC):

A single hand-curated stylesheet plus one rendered component
(`status_badge`). Component families: app chrome
(`.app-header`, `.app-nav`, `.app-main`, `.app-footer`), auth
(`.auth-card`), cards, forms, tables, buttons, banners, badges,
layout primitives (`.stack`, `.row`, `.grid-cards`),
confirmation screens (`.confirm-shell`), empty states, dashboard
primitives (`.sparkline`, `.recent-event-list`), setup wizard
(`.setup-lang-picker`, `.setup-step-indicator`), tabs
(`.me-tabs`).

**Utility classes** (RFC 067, CI gate `inline-style-bound` ≤ 20):

`.mt-*`, `.mb-*`, `.gap-*`, `.center`, `.items-center`,
`.justify-between`, `.max-w-card`, `.max-w-narrow`,
`.text-caption`, `.text-small`, `.fw-medium`, `.color-accent`,
`.color-danger`, `.flex-1`, `.flex-0-auto`, and a handful of
patterned classes (`.kv-label-cell`, `.button-reset`,
`.clickable-block`, `.radio-hint`, `.center-pad-*`, `.ul-indent`,
plus composites `.row-gap2-center`, `.row-gap3-center`,
`.gap1-center`).

**Two shells** (`layout.rs`):

- `Shell` — authenticated admin and self-service pages
  (header + nav + sign-out + main + footer)
- `AuthShell` — login, MFA challenge, password change outside the
  tab shell, setup wizard, error pages (brand + centred card +
  footer)

**Footer accessibility badges** are passive informational chips
(`<ul role="note">` / `<li class="app-footer__a11y-item">`) — they
state the app's commitment to keyboard / screen-reader / contrast
support but are not interactive (v0.48.2). The tagline `sui-id ·
静かで、凛として、やさしい ID 基盤を。` is rendered restrained:
caption-size, muted, 75 % opacity.

**Responsive** is a single breakpoint `@media (max-width: 768px)`
that switches the nav to horizontal-scroll, the footer to a
single column, and reduces main padding. Anything narrower than
~480 px still uses the desktop layout. Table cells default to
`white-space: nowrap` with `.cell-wrap` as the opt-out for
free-form text columns.

**Client-side JavaScript** is three small hand-written files
served from `/static/*` to satisfy CSP `script-src 'self'`:

| File | Purpose |
|---|---|
| `theme-init.js` | Theme `localStorage` resolution + listener attachment |
| `copy.js` | Delegated `data-copy="…"` click handler (RFC 028) |
| `logout-csrf.js` | Populates the sign-out form's hidden CSRF input from cookie |

No Wasm, no build step, no third-party CSS, no fonts.

---

## 18. Implementation constraints

- Rust 2024 Edition.
- `mod.rs` is not used; the umbrella `foo.rs` + sibling `foo/`
  pattern is the policy throughout.
- Dependencies stay thin.
- Heavy convenience features are adopted only after explicit review.
- Useless abstractions are avoided.
- Layers with no payload are avoided.
- No single crate accumulates outsized responsibility.
- Unit-testable design is the default.
- Business logic is separable from side effects.
- `unsafe_code = "forbid"` is workspace-wide.
- Login / MFA / lockout response times are equalised.
- Column-level seal / open uses the order `(key, plaintext, aad)`.
- Audit event names are dot-delimited lower-case.
- WebAuthn runs only on HTTPS or `localhost`.
- Session cookie is `HttpOnly` + `SameSite=Lax`; the `Secure` flag
  is configuration-controlled.
- **Inline JavaScript and inline event handlers (`onclick=` etc.)
  are forbidden in rendered HTML.** CSP defaults to
  `script-src 'self'`; client-side behaviour ships as external
  files in `crates/sui-id/static/`. (v0.48.1 hardening after a
  real-environment regression.)
- Inline `style="…"` is bounded by CI at 20 occurrences total in
  `crates/sui-id-web/src/pages/**`. Repeated inline styling is
  promoted to a utility class.

---

## 19. Documentation policy

All documentation and code comments are written in **English**.

### 19.1 Audiences

- New operators (introduction)
- Application developers (integration)
- Site reliability / operators (deployment, backup, key custody)
- Security reviewers / auditors
- Future maintainers

### 19.2 Structure

- Quick-onboarding content stays brief.
- Deeper material is layered behind for those who want it.
- Operation, recovery, and backup details are documented.
- Security caveats are surfaced, not buried.

### 19.3 Entry-document discipline

- The entry document is not bloated.
- It still does not omit critical material.
- It is optimised for time-to-useful-state on first read.

### 19.4 File layout

These files are present at the repository root and follow stable
conventions:

- `README.md`
- `ROADMAP.md`
- `CHANGELOG.md`
- `LICENSE` (Apache-2.0)
- `NOTICE`
- `.github/SECURITY.md`, `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`
- `.github/ISSUE_TEMPLATE/{bug_report,feature_request,question,config}.yml`
- `.github/workflows/{ci,audit}.yml`
- `.vscode/{extensions,settings}.json`
- `docs/threat-model.md`
- `docs/ui-ux-contracts.md`
- `docs/src/` (mdbook-compatible content)
- `examples/dev-seed.toml`

`README.md` does not bloat; depth lives under `docs/`.

### 19.5 `README.md` layout

1. Hero — badges + tagline
2. Overview (brief)
3. Why / when — use cases (brief)
4. Quick Start
5. Design Notes (3–5 lines of philosophy, not a feature dump)
6. Pointer to full documentation + links to key chapters

License attribution lives in `LICENSE` / `NOTICE` and as GitHub
badges; the README does not duplicate licence prose.

### 19.6 `README.md` link strategy

To stay valid on crates.io (where relative paths 404):

- Images: absolute
  `https://raw.githubusercontent.com/<owner>/<repo>/main/...`
- Files (LICENSE etc.): absolute
  `https://github.com/<owner>/<repo>/blob/main/...`

### 19.7 `docs/` organisation

- Group related material into subfolders.
- File naming is consistent.
- The hierarchy maps to the questions readers actually ask.

---

## 20. RFC lifecycle

The codebase has accumulated **60+ implemented RFCs** by v0.48.4.
The lifecycle policy lives in `rfcs/000-rfc-lifecycle-policy.md`.

### 20.1 When to write one

Any change that touches:

- A cross-cutting contract (UI/UX, state machines, error mapping)
- Public API surface (`render_*`, handler routes, DTOs)
- Token vocabulary in the design system
- Migration / backup compatibility
- Security-sensitive behaviour
- More than one crate at once

…justifies a dedicated RFC. Local refactors and obvious bug fixes
do not.

### 20.2 States

- **Proposed** — under review; lives in `rfcs/proposed/`.
- **Implemented** — shipped in a tagged release; moves to
  `rfcs/done/` with a `**Status.** Implemented (vX.Y.Z)` line.
- **Withdrawn / Superseded** — kept in `rfcs/done/` with status
  noted; never deleted.

### 20.3 Versioning

Each release that ships an RFC documents it in both `CHANGELOG.md`
(full prose) and `ROADMAP.md` (one-line row). The RFC file
references the shipping version.

### 20.4 Numbering

Sequential, three-digit, never re-used. Numbers come from the next
available slot in `rfcs/`.

---

## 21. CI invariants

Every PR runs the following gates (`.github/workflows/ci.yml`).
None of them is optional and none may be regressed:

| Job | Origin | Check |
|---|---|---|
| **build + test** | always | `cargo build`, `cargo test`. Warnings → error. **228/228 tests** is the current floor. |
| **fmt + clippy** | always | `cargo fmt --check`, `cargo clippy -D warnings`. |
| **text-leak invariants** | RFC 048, widened RFC 051 | `grep -rEn '>t\.[a-z_0-9]+<' crates/` must return empty. (Leptos `view!` macros render bare identifiers outside `{}` as literal text. Forty-eight sites leaked at v0.41.0; the gate has held 0 since v0.42.0.) |
| **css-tokens** | RFC 049 | Every `var(--name)` referenced anywhere in `crates/` resolves to a `--name:` declaration in `tokens.rs` or `components.rs`. |
| **semantic-palette-parity** | RFC 061 | The 12 semantic-palette token names (`danger / warning / success / info` × `default / subtle / fg-on`) each appear three times in `tokens.rs` (light, dark, auto-dark). |
| **inline-style-bound** | RFC 067 | `grep -rEohn 'style="[^"]*"' crates/sui-id-web/src/pages/ --include='*.rs' \| wc -l` ≤ 20. |

A PR that increases any of these counters must either decrease
them again before merge or motivate the change with a new RFC.

---

## 22. Verification phase

v0.48.0 closes Phase F of the hardening arc (the structural
release sequence that built the design system, split oversized
files, and bounded inline style). The project subsequently
**enters a verification phase**.

### 22.1 What the verification phase is for

Actual-environment testing surfaces classes of issues that
documentation review and unit testing miss:

- CSP enforcement breaking inline JavaScript only on real browsers
  (v0.48.1).
- Sign-out flow regressing into a redirect loop because of (1)
  (v0.48.1).
- 401 page lock-out loop after server restart (v0.48.1).
- `::selection` colour being technically WCAG-conformant yet
  practically invisible (v0.48.2).
- Hardcoded English literals surviving a Phase F file-split move
  (v0.48.2).
- Setup token UX requiring copy-paste from stderr (v0.48.4).
- ID token missing the `email` claim that relying parties expect
  (v0.48.3).

These were all real findings, each fixed in a targeted release.

### 22.2 No v1.0 tag during verification

**No tag beginning with `v1` is scheduled.** The verification phase
ends only when external review, soak time, and integration
verification combine to produce sufficient confidence. The
project owner has been explicit on this point: "rc / pre / beta
are still v1 designations and they are not scheduled."

Releases during the verification phase use `v0.48.x` and onward
sequential numbering.

### 22.3 Release mechanics

Each release ships as `sui-id-vX.Y.Z.tar.gz` from
`/mnt/user-data/outputs/`. The archive is built with
`tar --exclude='target' --exclude='.git' --exclude='sui-id-v*'`
to keep stray nested release directories out of the archive
(a contamination pattern that polluted v0.43–v0.48.0 archives
before being noticed at v0.48.1; corrected from v0.48.2 onward).

---

## 23. Acceptance criteria

The system is functionally complete when:

- Uninitialised state directs the operator to setup.
- The first administrator can be created (12-char minimum password).
- Initialised state transitions cleanly to normal operation.
- OIDC Discovery serves correct values.
- JWKS returns multiple key generations.
- Authorization Code + PKCE (S256) authenticates.
- `redirect_uri` is enforced exactly.
- Access and refresh tokens issue, rotate, revoke correctly.
- Refresh-token theft revokes the whole family.
- RP-Initiated Logout works.
- Token Introspection / Revocation work.
- Admins can suspend / delete users.
- Admins can manage clients (register / update / delete /
  rotate secret).
- MFA (TOTP / passkey) registers, authenticates, deregisters.
- Recovery codes issue, consume, regenerate.
- Step-up authentication gates the operations it should.
- Session idle-timeout and concurrent-cap behave as configured.
- i18n follows the resolution chain (ja / en).
- The setup wizard supports an explicit language picker.
- The setup token is supplied via URL parameter, not a text field.
- ID tokens carry `email` / `email_verified` when scope includes
  `email`.
- Mail features work only when SMTP is configured.
- HIBP enforces correctly in each of off / warn / block.
- Master-key rotation completes offline with no partial state.
- The audit-log hash chain is unbroken.
- No secrets appear in any log.
- Login / MFA / lockout response times are equal.
- Restart preserves all state.
- Backup and restore round-trip.
- The runtime file set stays small.
- No dangerous defaults exist.
- No `unsafe` Rust is present.
- `--dev` startup is immediately usable.
- CSP enforcement does not break any user-facing feature.
- All HTML 401 responses redirect, not render a page that
  loops back.
- The UI degrades sensibly on mobile (single breakpoint at 768 px).
- Workspace compiles with **0 warnings**.

---

## 24. Decision criteria

When a design question is ambiguous, decisions follow:

- Is it spec-compliant?
- Is it actually needed?
- Does it preserve safety?
- Does it keep operations simple?
- Will a first-time operator understand it?
- Does it add files or concepts unnecessarily?
- Is it future-extensible?
- Does it preserve local-first operation?
- Does it preserve timing equivalence?
- Is it auditable?
- Are dev-mode and production identical at the cryptographic layer?

The architect of any new UI/UX work should also consult
`docs/ui-ux-contracts.md` for cross-cutting rules
(screen-relation map, dangerous-operation pattern, state-word
vocabulary, audit-row copy).

---

## 25. Final policy

`sui-id` favours quiet trustworthiness over visible flash. Build
**the smallest thing that does not break** before reaching for
features. Convenience matters, but protect what must be protected
first. Hold this disposition through every release of the
verification phase and beyond.

---

## Appendix A — Release history overview

| Range | Phase | Outcome |
|---|---|---|
| v0.1 – v0.29 | Initial implementation | Core OIDC, admin panel, MFA, audit, dev mode |
| v0.30 – v0.41 | UI/UX hardening preparation | Audit log, dashboard, settings tabs, dangerous-op pattern, i18n, self-service tabs, consent screen |
| v0.42 (Phase A) | Text-leak + token-freeze + admin-chrome i18n | RFCs 048 / 049 / 050 |
| v0.43 (Phase B) | i18n completeness + status badge | RFCs 051 / 052 / 053 |
| v0.44 (Phase C) | Self-service unification + recovery-codes count | RFCs 054 / 055 / 056 / 057 |
| v0.45 (Phase D) | Step-up enforcement + confirm screen + audit notes | RFCs 058 / 059 / 060 |
| v0.46 (Phase E) | Semantic palette + card variants + dashboard signal/noise | RFCs 061 / 062 / 063 / 064 |
| v0.47 – v0.48.0 (Phase F) | Module split + inline-style discipline | RFCs 065 / 066 / 067 / 068 |
| **v0.48.1 onward** | **Verification phase** | Real-environment bug fixes; UX improvements; no v1 tag scheduled |

## Appendix B — Reference index

- `docs/ui-ux-contracts.md` — cross-cutting UI/UX rulebook
- `docs/threat-model.md` — 12 + 8 + 3 model
- `rfcs/000-rfc-lifecycle-policy.md` — RFC operations
- `rfcs/done/049-css-token-vocabulary-freeze.md` — token freeze
- `rfcs/done/061-semantic-palette-extension.md` — semantic palette
- `rfcs/done/067-inline-style-discipline.md` — utility-class bound
- `.github/workflows/ci.yml` — the four named CI gates
- `crates/sui-id-web/src/layout.rs` — `Shell` / `AuthShell`
- `crates/sui-id-web/src/tokens.rs` — design tokens
- `crates/sui-id-web/src/components.rs` — component CSS + utility classes
- `crates/sui-id/static/` — three hand-written client JS files

*End of specification.*
