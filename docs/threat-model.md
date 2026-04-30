# Threat model

This document describes the threats sui-id is designed to resist, the threats
it does **not** try to resist, and the assumptions an operator must hold up
on their end of the deployment for the design to work.

It is written in plain language and does not use STRIDE or LINDDUN
terminology, but the categories below cover the same ground.

## Assumptions

For sui-id to behave as advertised, the operator must:

1. **Run on a host they control.** sui-id has no defence against an
   attacker with root on the same machine.
2. **Hold the master encryption key separately from the database file.**
   The whole point of column-level encryption is that a stolen `.sqlite`
   does not yield refresh tokens or signing keys. If the key file ends up
   in the same backup archive as the database without separate access
   control, that property is lost.
3. **Terminate TLS in front of sui-id, or run on a trusted private
   network.** sui-id binds plain HTTP. Browser cookies and bearer tokens
   travelling over an untrusted network would be visible to anyone on
   that network.
4. **Configure `server.trusted_proxies` correctly when behind a reverse
   proxy.** A misconfigured trusted-proxy list lets any caller spoof
   their IP for rate-limit purposes.
5. **Keep the host clock approximately accurate.** JWT validation
   compares wall-clock times. A host whose clock is hours off may issue
   tokens that look already-expired or never-expires to relying parties.
6. **Back up the master key.** Losing the key permanently destroys access
   to encrypted columns. sui-id has no key-recovery mechanism by design.

## Adversaries we plan for

### A1. Network attacker on the path between client and proxy

Examples: a coffee-shop wifi attacker, a misbehaving ISP, an attacker who
has compromised an upstream router.

What we do:

- Bearer access tokens are short-lived (15 minutes by default) and signed
  with Ed25519. Even if intercepted, they expire fast and cannot be
  forged.
- Refresh tokens are opaque random strings, sealed at rest. They rotate
  on every use: an intercepted token is invalidated as soon as the
  legitimate client uses it, and reuse of an old refresh token returns
  `invalid_grant`.
- The session cookie is `HttpOnly` and `SameSite=Lax`. Operators set
  `cookie_secure = true` behind HTTPS so the cookie never leaves an
  encrypted channel.

What the operator must do:

- Terminate TLS. sui-id does not.

### A2. Network attacker between proxy and sui-id

Examples: an unprivileged process on the same host that can connect to
the loopback bind, a sidecar with broader network access than expected.

What we do:

- All authentication state is server-side; the cookie is just an opaque
  session id. There is no token in the cookie that a local snooper could
  replay across hosts.
- The master encryption key is kept in a file with `0600` permissions or
  in an environment variable. It does not enter the database file or the
  HTTP layer.

What the operator must do:

- Bind to `127.0.0.1` and run the proxy on the same host (the documented
  configuration), or use a private network the operator controls.

### A3. Attacker who has read-only stolen the SQLite file

Examples: a backup tarball was leaked, a snapshot of the volume was
exfiltrated, a cloud bucket was misconfigured.

What we do:

- Refresh tokens are stored as XChaCha20-Poly1305 ciphertext keyed off
  the master key. They cannot be reused.
- Signing key bytes are stored the same way. They cannot be used to
  forge access tokens.
- Passwords are stored as Argon2id hashes (PHC string). They are not
  reversible without an offline brute-force attack against each
  password.
- Client secrets are stored as Argon2id hashes too — even a confidential
  client's secret is not recoverable from the database.

What we do **not** do:

- Encrypt usernames, display names, or audit log entries. An attacker
  with a stolen database file can enumerate user accounts and see the
  history of administrative actions. This is a deliberate trade-off:
  encrypting usernames would prevent us from looking them up by name.

What the operator must do:

- Hold the master key file in a different trust domain than the database
  backup. A backup tarball that contains both files together is no
  better than a plaintext backup.

### A4. Attacker who has read-write stolen the SQLite file (writable mount)

Examples: an admin who has shell access on the host but not the master
key, a process that should only have read access but has been
misconfigured.

What we do:

- The audit log is append-only by code: sui-id never issues `UPDATE` or
  `DELETE` against `audit_log`. An attacker who can write the file
  directly can of course tamper with it; we make sure sui-id itself
  cannot.
- All the protections from A3 still hold: even with write access, the
  attacker cannot mint access tokens or forge refresh tokens without
  the master key.

What we do **not** do:

- Detect tampering. There is no signed audit log, no Merkle chain over
  rows, no off-host journal. Operators who need that should ship the
  audit log to an external WORM destination.

### A5. Online password guessing against `/admin/login`

What we do:

- Argon2id with conservative parameters (m=64 MiB, t=2, p=1) makes each
  guess expensive.
- Per-IP rate limiting on `/admin/login` (default: 10 attempts per
  60-second window per IP) returns 429 with `Retry-After`.
- **Per-account progressive lockout** (since v0.16.0): the third
  consecutive password failure stamps a 30-second lock; the curve
  grows from there to a configurable cap (default 24 hours,
  selectable from `15min`, `1h`, `4h`, `12h`, `24h`, `48h` via
  `[security] max_lockout`). A successful sign-in clears the
  counter. The first two failures cost nothing — that's the typo
  budget for legitimate users.
- Login outcomes are written to the audit log so operators can see
  patterns. The dedicated `auth.login.locked` event distinguishes
  "we just locked an account" from ordinary "wrong password" so a
  SIEM can alert on bursts of locks.
- Login responses for unknown usernames, disabled users, locked
  accounts, and wrong passwords all run an Argon2id verify
  (against a dummy hash where there's no real one to verify against)
  so wall-clock timing does not distinguish the four cases.
- Admin-initiated unlock with `sui-id admin unlock-user --username NAME`
  exists for the case where a real user has been locked out and
  needs to recover before the auto-unlock window expires.

What this trades off:

- **DoS amplification**: an attacker who knows a username can lock
  that account by submitting wrong passwords. We accept this
  trade-off because (a) the per-IP rate limit on `/admin/login`
  caps how fast an attacker can run up the failure count, (b) an
  admin can clear the lock from the host, and (c) the fixed
  `max_lockout` cap keeps the worst-case lockout bounded — at
  default settings, a real user is at most one day from being able
  to sign in again on their own. Earlier versions of sui-id (≤
  v0.15.0) deliberately omitted lockout for exactly this reason;
  the v0.16.0 design, with a configurable cap and an admin
  unlock command, brings the trade-off back in our favour.

### A6. Online password guessing or token grinding against `/oauth2/token`

What we do:

- Per-IP rate limit on `/oauth2/token` (default: 60 per 60 seconds).
- Authorization codes are single-use, hashed (SHA-256) before storage,
  and live for 60 seconds.
- PKCE is mandatory and validated in constant time.
- Refresh tokens are 32 random bytes (256 bits) drawn from the OS RNG;
  guessing one is not practical.

### A7. Cross-site request forgery on the admin UI

What we do:

- Every admin page sets a `sui_id_csrf` cookie carrying a 32-byte
  random token; every form rendered by that page embeds the same
  token as a hidden `_csrf` field. On POST, sui-id reads both and
  compares them in constant time. A missing or mismatched token
  returns 403.
- The session cookie is `SameSite=Lax` and `HttpOnly`. Top-level
  navigations from a malicious site cannot carry the cookie to a
  sensitive POST, and JavaScript on the page cannot exfiltrate it.
- Admin actions are POST forms with no GET-side equivalents. A
  malicious image tag on another origin cannot cause a state change.
- The CSRF cookie is intentionally **not** `HttpOnly` so the page can
  read its own token. The cookie alone is harmless — only paired
  with a matching form field on a session-authenticated request does
  it grant anything.

What we do **not** do:

- Apply CSRF to `/oauth2/*`. Those endpoints are protocol traffic
  between the relying party's backend and sui-id, not user-facing
  forms, and CSRF does not apply to them. They are protected by
  client authentication (`client_secret`), PKCE, and authorization
  code single-use semantics instead.

### A8. Open redirect via `redirect_uri` or `post_logout_redirect_uri`

What we do:

- Both endpoints accept only URIs that have been pre-registered against
  a client. No partial matches, no wildcard matches.
- `redirect_uri` must be `https://` except for `http://localhost` /
  `127.0.0.1` / `[::1]`. Fragments are forbidden.
- `post_logout_redirect_uri` requires an `id_token_hint` (or
  `client_id` fallback). The URI must match a `redirect_uris` entry on
  that client, otherwise sui-id ignores the parameter and shows a
  static "Signed out" page.

### A9. JWT confusion / signing-key compromise

What we do:

- We accept only `EdDSA` (Ed25519). The verifier rejects any other
  `alg`, so an attacker cannot downgrade to `none` or to symmetric HS256.
- The `kid` header is required. Verification looks up the kid in JWKS,
  not from any caller-supplied input.
- Private keys live sealed in the database. With the master key
  separated, even a stolen database does not yield signing capability.
- Administrators can rotate the signing key from `/admin/signing-keys`.
  A rotation publishes the new key as the active signer, demotes the
  previous key to retired status, and keeps it in JWKS so already-issued
  tokens continue to verify until they expire. Once expired, the
  retired key can be deleted.

What we do **not** do (yet):

- Automatic / scheduled rotation. Rotation today is operator-driven.

### A10. Replay of an access token after revocation

What we do:

- Access tokens are short-lived (15 minutes by default), so revocation
  has a bounded window.
- Logout and account suspension revoke all of the user's outstanding
  refresh tokens and sessions, so the user cannot get a *new* access
  token after revocation.

What we do **not** do:

- Revocation lists for already-issued access tokens. The standard answer
  is short access-token lifetimes plus refresh-token rotation, which is
  what we do. Operators who require immediate revocation should
  configure a smaller `tokens.access_lifetime_secs`.

### A11. Compromise of a single password

What we do:

- The `/admin/profile` page lets every account opt in to one or both
  of two MFA factors:
  - **TOTP** (RFC 6238, HMAC-SHA1, 30-second window, 6 digits) with
    8 single-use Argon2id-hashed recovery codes.
  - **WebAuthn / passkeys** — hardware-backed credentials registered
    via the browser's `navigator.credentials.create()` API. A user
    may register multiple passkeys (security key + platform
    authenticator + recovery key on a different device).
- Once *either* factor is enabled, password authentication alone
  never produces a session. The user must also pass the second
  factor at `/admin/login/mfa`. The challenge page accepts a TOTP
  code, a recovery code, or a passkey assertion — whichever the
  user has.
- WebAuthn is phishing-resistant in a way TOTP is not: the browser
  binds the credential to the relying-party id (`rp_id`), so a
  fake login page on a look-alike domain cannot trick the
  authenticator into producing a usable assertion.
- TOTP secrets and the entire `Passkey` value (public key, signature
  counter, attestation metadata) are sealed under the master key in
  storage. The plaintext exists only briefly during the relevant
  ceremony.
- A `last_used_step` cursor stops a successful 6-digit TOTP code
  from being replayed within its 30-second window.
- WebAuthn ceremonies persist their in-flight state in a
  `webauthn_pending` table behind the master key. The
  `danger-allow-state-serialisation` feature of webauthn-rs is
  used purely for that internal storage; the state never crosses
  a network boundary.

What we do **not** do:

- Force MFA on every account. The operator chooses, and so does each
  user. A future release may add an "all admins must have MFA"
  policy switch.
- Implement WebAuthn attestation verification beyond what the
  default `start_passkey_registration` flow does. Synchronised
  passkeys (Apple iCloud Keychain, Google Password Manager, etc) by
  design do not produce trustworthy attestation; the attested-passkey
  flow exists in webauthn-rs but is not exposed in sui-id today.

For users who have lost every factor — password reset, TOTP
authenticator, recovery codes, *and* every passkey — sui-id provides
an administrator-initiated reset at `/admin/users/{id}/mfa-reset`.
This lifts every second factor for the target user, audit-logged with
the actor's id and a breakdown of what was removed. The reset does
not revoke active sessions; an operator who wants a hard logout in
addition should follow with disable-and-re-enable on the same row.

### A12. Compromise or vulnerability in a third-party authentication library

What we do:

- The WebAuthn implementation depends on `webauthn-rs` 0.5, the safe
  high-level wrapper from the kanidm project. We use the
  high-level `Webauthn::start_passkey_registration` /
  `finish_passkey_registration` and `start_passkey_authentication` /
  `finish_passkey_authentication` API only. The lower-level
  `webauthn-rs-core` ships with explicit warnings telling
  integrators not to call it directly; we don't.
- All other cryptographic primitives are kept narrow and visible:
  Argon2id (RustCrypto `argon2`), Ed25519 (`ed25519-dalek`),
  XChaCha20-Poly1305 (`chacha20poly1305`), HMAC-SHA1 (RustCrypto
  `hmac` + `sha1`). These are widely audited and have small,
  well-understood APIs.
- Production builds should track the upstream advisory feed for
  these crates. `cargo audit` against the published crate version
  is part of the recommended pre-deploy checklist, and the upstream
  CI runs the same scan on every push and on a weekly schedule
  (see `.github/workflows/audit.yml`). At the time v0.10.2 shipped,
  the dependency tree had **zero** known vulnerabilities and one
  informational warning (`paste`, an unmaintained transitive of
  the Leptos framework) which is not directly exploitable.

What we do **not** do:

- Audit the transitive dependency graph of `webauthn-rs` ourselves.
  Notably, `webauthn-rs` pulls in OpenSSL via
  `webauthn-rs-core` for some cryptographic operations; an OpenSSL
  vulnerability would surface here. This is the cost of using a
  battle-tested library — we accept it as preferable to writing the
  cryptographic verification ourselves. The `cargo audit` integration
  in CI is the mitigation: an OpenSSL CVE that lands in RustSec is
  flagged on the next push and the next weekly scan.
- Pin the patch version of `webauthn-rs`. The `Cargo.lock` is
  reproducible, but operators rebuilding from source should consider
  whether they want to re-pin or to accept whatever the latest
  0.5.x patch is at build time.

### A13. Attacker who intercepts a backup tarball in transit

Threat: an operator copies a backup off-host (cloud storage, removable
media, email to a colleague, transfer to a new server during
migration). Somewhere along that channel the file is captured.

A plain backup tarball contains the master key. An attacker who
captures it has compromised the entire installation as completely
as if they had stolen `/var/lib/sui-id/sui-id.key` and
`/var/lib/sui-id/sui-id.sqlite` directly from the live host.

What we do:

- The `--encrypt` option on `sui-id backup` wraps the tarball in an
  XChaCha20-Poly1305 envelope. The key is derived from a
  passphrase via Argon2id with parameters tuned to take roughly a
  second on contemporary hardware (m_cost = 64 MiB, t_cost = 3,
  p_cost = 1; well above the OWASP minimum). Salt and nonce are
  generated fresh per backup; both are stored in the envelope.
- Restoring or `verify-backup`-ing an encrypted backup requires
  the matching passphrase. The Poly1305 tag rejects tampering.
- The envelope format includes a magic header (`SUIDIDBK`) and a
  format-version field, so a future incompatible change can be
  detected and refused cleanly rather than producing garbled
  output.

What an operator must still do:

- **Use `--encrypt` for backups that will leave the host's trust
  boundary.** A plain backup is fine for a same-host or same-trust-
  boundary destination; an encrypted backup is what should ride
  rsync-to-S3, an off-site disk, or a transfer to a migration host.
- **Send the passphrase out-of-band.** The whole point is that the
  passphrase travels through a different channel than the file. If
  both end up on the same compromised system, the encryption gives
  no protection.
- **Choose a passphrase with enough entropy.** Argon2id buys time
  against brute force, but a 4-word passphrase will not survive
  determined offline grinding. The deployment guide recommends
  generating a passphrase from `head -c 32 /dev/urandom | base64`
  and keeping it in a password manager.
- **Lose the passphrase, lose the backup.** sui-id has no recovery
  mechanism for forgotten backup passphrases. This is the same
  trade-off as for any encrypted archive.

What we do not do:

- Implement any kind of split-key, threshold, or HSM-backed
  key-derivation for backup envelopes. The single-passphrase model
  fits the self-hosted, single-operator scope; teams that need
  fancier custodianship will have already invested in something
  like Vault and can wrap the plain tarball with that instead.
- Verify a passphrase before deriving the key. This is a deliberate
  trade-off: a user-friendly "wrong passphrase" check that did not
  also serve as a brute-force oracle would require a separate
  authenticator. The Poly1305 tag is the authoritative check —
  it gives a clear error on the wrong passphrase, without leaking
  any signal stronger than "decryption failed".

## Adversaries we do not plan for

These are out of scope. Either the threat is genuinely better handled
elsewhere in the stack, or sui-id is too small to address it.

- **A nation-state-level adversary with persistent access to the host.**
  Once the master key and database file are simultaneously available to
  an attacker, the design's security properties are gone.
- **Side-channel attacks against the host CPU.** Spectre-class issues
  are the responsibility of the kernel and hypervisor.
- **Phishing of administrators.** The user experience for sign-in is
  conventional. An admin who pastes their credentials into a
  look-alike domain is past sui-id's reach. MFA (on the roadmap) will
  raise the bar here.
- **Compromise of the operator's TLS certificate.** sui-id never sees
  the certificate; this is the proxy's responsibility. A leaked
  certificate is recoverable through normal cert-rotation procedures
  in the operator's PKI.
- **Compromise of the relying party.** Sui-id authenticates *users to
  RPs*; it cannot defend an RP whose own backend is breached.
- **Cryptographic breaks of the underlying primitives.** Argon2id,
  Ed25519, XChaCha20-Poly1305, and SHA-256 are assumed secure. If any
  of those falls, this is a coordinated industry-wide problem, not a
  sui-id-specific one.

## What to monitor

Given the design, the highest-value signals an operator can collect are:

1. **The audit log.** Tail it. Repeated `auth.login.failure` from the
   same target is the obvious flag, but also watch for unexpected
   `client.create`, `user.disable`, and `user.reset_password` events.
2. **HTTP 429 responses.** A sustained rate of 429s on `/admin/login`
   means somebody is grinding; on `/oauth2/token` it usually means a
   misbehaving client.
3. **Disk usage of the SQLite file.** Should be bounded; runaway growth
   suggests the GC task isn't running or someone is creating many
   short-lived clients.
4. **Unexpected server-side errors.** Any 5xx from sui-id should be rare
   in a healthy deployment. The structured log line carries a
   `request_id` that the user can quote when reporting a problem.

## Reporting

If you think you have found a way around any of the protections above —
or a property the design should provide and does not — please follow
[`.github/SECURITY.md`](../.github/SECURITY.md) and **do not** file a
public issue.
