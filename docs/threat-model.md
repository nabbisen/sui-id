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
- Login outcomes are written to the audit log so operators can see
  patterns.
- Login responses for unknown usernames also run a dummy Argon2 verify
  so timing does not distinguish "no such user" from "wrong password".

What we do **not** do:

- Lock accounts after N failures. Account lockout is a denial-of-service
  amplifier (an attacker can lock out any user they know the username
  of). Rate-limit the attacker, not the victim.

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

- The session cookie is `SameSite=Lax`. Top-level navigations from a
  malicious site cannot carry the cookie to a sensitive POST.
- Admin actions are POST forms with no `GET`-side equivalents. Reading a
  malicious image tag cannot cause a state change.

What we do **not** do (yet):

- Per-form CSRF tokens. `SameSite=Lax` is sufficient against the
  classic attack but is brittle if a future change introduces a
  cross-origin POST. A future release should add a synchronizer token
  on the admin forms; this is on the roadmap.

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
