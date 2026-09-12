# sui-id — Development Specification

*v4 — reflecting the v0.77.0 codebase (2026-09-12). Supersedes v3
(v0.48.4). Under RFC 098 this document is a synthesis of policy and
principle, not a source: inventories were removed in v4 and each section
that held one now names where the authoritative copy lives.*

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

- Distributed / clustered operation
- Large-scale multi-tenant operation — the single-realm model is a
  constraint, not an oversight; per-tenant isolation is RFC 025, post-1.0
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
- Alternative SQL backends beyond the `Backend` trait (RFC 009 step 1);
  the SQLite implementation is the supported one
- A user-facing theming API — CSS tokens are the maintainer's vocabulary,
  not an operator interface
- A plugin system — sketched in RFC 005, not scheduled

*Amended in v4:* "Social login" and "External IdP federation" were listed
here through v3. Both shipped — upstream OIDC federation is RFC 004 and
read-only LDAP user sources are RFC 005 — so the boundary they described
no longer exists. The current non-goals are `ROADMAP.md` §Constraints and
non-goals (pre-1.0), which is authoritative where this list and it differ.

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
- **Federation provider** — a configured upstream OIDC provider that
  sui-id authenticates users against as a relying party (RFC 004).
- **User source** — the origin of a user record: the local store, or a
  read-only external directory reached over LDAP (RFC 005).
- **Client registration token** — a single-use credential that authorises
  a third party to register an OIDC client without an administrator
  (RFC 008).
- **Metrics token** — the bearer credential guarding the Prometheus
  metrics endpoint; stored hashed and rotated from the CLI (RFC 006).

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
- OAuth 2.0 Dynamic Client Registration (RFC 7591) — RFC 008
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
- Prefer the minimal OIDC subset needed for federation. sui-id is an OP
  and, since RFC 004, a relying party to upstream OIDC providers; the
  same preference governs both roles

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
- Federation client secrets (RFC 004)
- LDAP bind credentials (RFC 005)
- The metrics bearer token (RFC 006)
- Client registration tokens (RFC 008)

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

The threat model is `docs/threat-model.md`, and it is the only place
threats, defensive properties and known limits are stated (RFC 098 §4
rule 7). This specification does not summarise it; a summary is a copy,
and the one this section carried through v3 had drifted.

---

## 8. Tech stack

### 8.1 Required

- **Rust 2024 Edition**
- **TOML configuration**
- **Cargo workspace structure**

The toolchain floor and the build matrix it is verified against are RFC 093
§Gate Matrix v1 and `rust-version` in the workspace `Cargo.toml`. Pinning
them here would be a second copy of a version two lanes already enforce.

### 8.2 Principal libraries

Dependencies are chosen for thinness and reviewed before adoption (§8.3),
and the crates that carry the security-relevant work — password hashing,
column encryption, signing, WebAuthn — are deliberately few and boring.

The authoritative list is `[workspace.dependencies]` in `Cargo.toml`. The
table this section carried through v3 had drifted: it named `ureq` for the
HIBP client, which this workspace does not depend on.

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

A Cargo workspace of six crates (§9.1), with documentation under `docs/`,
governance under `rfcs/`, and the machine-consumed gate inputs under `ci/`.
The principle is that a reader should be able to guess where something
lives from what it does.

The tree itself is authoritative and is not transcribed here: `rfcs/`'s
layout is RFC 000, the repository layout is `README.md` §Project layout,
and the CLI surface is `sui-id --help`. The v3 transcript of this tree had
drifted in every one of those three directions.

### 9.1 Crate responsibilities

| Crate | Responsibility |
|---|---|
| **`sui-id`** | Executable. `main.rs`, axum bootstrap, router, asset embedding, config load, the CLI subcommands (`sui-id --help`), `--dev` mode (`src/runtime/dev_mode.rs`), HTTP handlers (`src/http/handlers/`). Static JS lives in `crates/sui-id/static/`. |
| **`sui-id-core`** | Use-case layer free of handler/HTTP concerns. Authn/authz, OIDC code/token/discovery/JWKS/introspection/revocation, password hashing, JWT signing, MFA (TOTP + WebAuthn + recovery codes), session lifecycle (idle timeout, concurrent-session cap), lockout, step-up, mail dispatch, HIBP client, master-key rotation, domain error types. |
| **`sui-id-store`** | SQLite persistence, migrations, column-level encryption, repository implementations, audit-log persistence (SHA-256 hash chain). |
| **`sui-id-web`** | Leptos SSR. Admin / setup / settings / self-service UIs, design tokens (`tokens.rs`), component CSS (`components.rs`), layout shells (`layout.rs`), per-screen render functions (`pages/`). |
| **`sui-id-i18n`** | `Locale` enum + `Strings` struct, per-locale files under `locale/`, Accept-Language negotiation. Which locales are selectable is `Locale::ALL`; §11.10 states the promotion rule. |
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
- Since RFC 004 sui-id is also a **relying party** to upstream OIDC
  providers, and since RFC 008 it accepts **dynamic client registration**
  under a single-use token. Both are documented in
  `docs/src/reference/oidc-api.md`.
- The Prometheus metrics endpoint (RFC 006) is guarded by a bearer token;
  see `docs/src/guides/operators.md`.

### 11.2 User management

- Create, list, disable / re-enable, delete users.
- Password reset (admin-initiated + email-initiated).
- Force-logout (single user or all).
- Logical delete is used where appropriate.
- HIBP check on password set (off / warn / block).
- A user may originate from the local store or from a read-only LDAP
  directory (RFC 005); `docs/src/guides/operators.md` covers configuring a
  user source and what remains local when one is in use.

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
- Administrator registration is not the only path since RFC 008: a third
  party holding a single-use registration token can register a client
  itself. `docs/src/reference/oidc-api.md` documents the endpoint and the
  token's issuance.

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

Users, credentials, clients, authorization codes, sessions, refresh-token
families, signing-key generations, WebAuthn credentials, the audit chain,
and the singleton settings rows. Identity is separated from credentials;
short-lived artifacts carry expiry; nothing security-relevant is stored
without a reason it can be read back.

The schema is authoritative and is not transcribed here:
`crates/sui-id-store/src/migrations/`. The v3 list had fallen seven tables
behind, and described consent as reserved for future expansion when it had
already shipped.

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

The audit log captures: who, when, what, outcome. Every administrative
action is covered, and a SHA-256 hash chain (each row hashes the canonical
bytes of the prior hash plus the current event) provides tamper evidence
within the trust boundary `docs/threat-model.md` states.

The event vocabulary is authoritative and is not transcribed here:
`ci/audit-coverage-matrix.md` is the gate input that G13 checks against the
source literals in both directions, and `docs/src/reference/audit-events.md`
is the reader-facing reference. The v3 list named six events against
fifty-four registered, and one of the six — `auth.refresh.family_revoked` —
was a name that never existed in the code.

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
  `--fg-on-{name}` (RFC 061)
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

**Utility classes** (RFC 067; inline `style=` is bounded, see §18):

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

**Client-side JavaScript** is a handful of small hand-written files served
from `/static/*` to satisfy CSP `script-src 'self'`. No Wasm, no build step,
no third-party CSS, no fonts; behaviour that cannot be server-rendered is
written by hand rather than imported. The files themselves are
`crates/sui-id/static/`, and the invariants that bind this section — token
resolution, semantic-palette parity, the inline-style bound, text leaks —
are `ci/ui-invariants.toml`, enforced as lane G12. The v3 table listed
three files, one of which no longer exists, and named two of those checks
as standalone CI gates that RFC 093 M1b C5 consolidated into G12.

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

The repository root carries the files a reader looks for by name —
`README.md`, `ROADMAP.md`, `CHANGELOG.md`, `LICENSE`, `NOTICE` — with
community and CI configuration under `.github/`, and depth under `docs/`
organised per §19.7. `README.md` does not bloat.

The file set itself is the authority: the tree, RFC 000 for `rfcs/`, and
§19.7 for `docs/`. The v3 enumeration had drifted; a list of filenames is
the fastest-rotting thing a specification can hold.

### 19.5 `README.md` layout

1. Hero — badges + tagline
2. Overview (brief)
3. Why / when — use cases (brief)
4. Quick Start
5. Design Notes (3–5 lines of philosophy, not a feature dump)
6. Pointer to full documentation + links to key chapters

License attribution lives in `LICENSE` / `NOTICE` and as GitHub
badges; the README does not duplicate licence prose.

### 19.6 Link form

RFC 098 §4 rule 6 decides this, in both directions: a page under
`docs/src/` reaches a file outside the book by absolute repository URL,
because mdBook rewrites a relative `.md` target to an `.html` page the
build never produces; every other tracked document links
repository-relative. G15 check (B) enforces both halves.

This reverses the v3 rule, which told authors to write absolute
`https://github.com/<owner>/<repo>/blob/main/...` URLs for files. Those are
invisible to the link gates, which is how `README.md`'s link to its own
threat model went unchecked. Images in `README.md` remain absolute
`https://raw.githubusercontent.com/...` URLs so the crates.io rendering
shows them.

### 19.7 `docs/` organisation

- Group related material into subfolders.
- File naming is consistent.
- The hierarchy maps to the questions readers actually ask.

---

## 20. RFC lifecycle

The codebase has accumulated **60+ implemented RFCs** by v0.48.4.
The lifecycle policy lives in
`rfcs/done/000-rfc-lifecycle-policy.md`.

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
- **Accepted** — design review is complete and repository-visible approval
  metadata is recorded; lives in `rfcs/accepted/`. This is the only
  implementation-eligible state, but coding remains prohibited until every
  declared implementation prerequisite has repository-visible passing
  evidence.
- **Implemented** — shipped in a tagged release; moves to
  `rfcs/done/` with a `**Status.** Implemented (vX.Y.Z)` line.
- **Withdrawn / Superseded** — kept in `rfcs/archive/` with status
  noted; never deleted.

Every new RFC declares whether security review is Required or Not required. A
Not-required classification includes a reason and named approver. Acceptance
records the date, approver, implementation owner, and independent design
reviewer where required. When security review is Required, shipment also
records the closure-review date, independent closure approver, and durable
evidence reference. The implementer cannot be the sole approver of a
security-sensitive design or its closure evidence.

The closure record uses `**Closure reviewed on.**`,
`**Closure approved by.**`, and `**Closure evidence.**`. The evidence is a
resolvable repository-relative durable artifact or review reference; a release
version alone is insufficient.

A material change to an Accepted RFC's security invariants, public behavior,
scope, or prerequisites moves it back to Proposed for review in one change:
Status, folder, active acceptance metadata, index, and inbound links are all
updated together. The prior decision remains in version-control history or an
explicit superseded-review note.

Each RFC distinguishes design-acceptance prerequisites, implementation-start
prerequisites, and closure prerequisites. Dependency edges therefore do not
silently block design review when they are intended only to sequence coding or
milestone closure.

### 20.3 Versioning

Each release that ships an RFC documents it in both `CHANGELOG.md`
(full prose) and `ROADMAP.md` (one-line row). The RFC file
references the shipping version. The roadmap does not reserve versions;
a checkpoint version may be assigned only after every RFC governing that
checkpoint is Accepted.

### 20.4 Numbering

Sequential, three-digit, never re-used. Numbers come from the next
available slot in `rfcs/`.

---

## 21. CI invariants

Every push runs a fixed set of blocking lanes. None is optional and none
may be regressed: a change that would breach one either fixes itself before
merge or motivates the breach with a new RFC. The lanes are not a
convention — the manifest they run from is itself gated, so a lane cannot
be quietly renamed, removed, or pointed at a different command.

The lane set is authoritative and is not transcribed here. RFC 093 §Gate
Matrix v1 owns G01–G12; RFC 094 and RFC 098 own their own lanes through
the multi-source registry; `ci/gate-inputs.toml` is the machine-readable
manifest that `scripts/ci-gate.sh` dispatches and that lane A3.4 verifies
against each owning RFC's table, byte for byte.

The v3 table listed six jobs and a floor of "228/228 tests". Both were
second copies: the lane set has more than doubled, and the test count is a
measurement, not a contract.

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

Releases are cut from `main` with the gates green; the procedure, including
what is published to crates.io and in what order, is
`docs/src/contributing/release-process.md`, and
the build and release-gate contract is RFC 093.

The v3 text described an archive assembled from a path in the authoring
environment. That was never a repository mechanism and could not be
followed by anyone else.

---

## 23. Acceptance criteria

Functional completeness is judged against the programme's own record, not
against a list kept here: `ROADMAP.md` §Programme outcomes states what each
milestone must deliver, and an RFC's closure prerequisites state what its
feature must satisfy before it ships.

The v3 checklist was a snapshot of v0.48.4's surface. It had no criterion
for federated sign-in, LDAP-sourced authentication, dynamic registration or
the metrics endpoint — four shipped subsystems — which is what a
completeness list looks like once it stops being maintained.

The standing criteria that are *policy* rather than inventory still hold
and are stated where they belong: no `unsafe` Rust and a zero-warning
workspace (§18, lanes G07/G07b/G08), timing equivalence on the
authentication paths (§6.3, §16.4), an unbroken audit chain (§15.2), no
secrets in any log (§6.2), and no dangerous defaults (§2.1).

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

## Appendix A — Release history

`CHANGELOG.md` is the release history, in full prose, and `ROADMAP.md`
carries the one-line row per shipped RFC (§20.3). The v3 appendix
summarised releases up to v0.48.1 and stopped; twenty-nine minor releases
and the whole security-assurance, UI-security and remediation arcs had
accumulated behind it.

*End of specification.*
