# Threat model

This document describes how sui-id thinks about the threats it
faces, what defences are in place, and where the boundaries of
those defences sit. It is current as of **v0.26.0** and reflects
every security-relevant feature shipped between v0.1.0 and that
release.

## Who this is for

Three audiences read this document for different reasons; the
prose tries to serve all three without forcing any one of them to
skip large fenced-off sections.

- **Operators and developers** running sui-id or building on
  top of it. Read Parts 1–3. Part 4 is optional reading; treat
  it as the appendix you reach for when something specific
  comes up.
- **Security auditors** evaluating sui-id for a deployment or
  for inclusion in a larger product. Read Parts 1–4 in order.
  Part 4 has the formal STRIDE breakdown and the more
  exhaustive attack-tree fragments.
- **Enterprise adopters** doing a vendor-due-diligence pass.
  Parts 1, 4 ("Detailed concerns") and 5 ("Known limitations")
  are the highest-density material; the threat-scenario
  walkthroughs in Part 2 are the gentlest entry point if the
  reviewer wants to verify a specific concern.

## Document layout

- **Part 1 — Foundations.** Scope, trust boundaries, the
  adversaries we model, asset taxonomy.
- **Part 2 — Threat scenarios.** Twelve scenarios, each
  describing an attacker goal, the path they would take, and
  the defences that block (or bound) the attack.
- **Part 3 — Defensive properties.** The architectural
  invariants that hold everywhere — column-encryption AAD
  binding, master-key separation, audit hash-chain
  monotonicity, etc.
- **Part 4 — Detailed concerns.** STRIDE-style breakdown,
  attack-tree fragments, compliance notes. Aimed at auditors
  and at enterprise-due-diligence readers; safely skippable
  for operators.
- **Part 5 — Known limitations and future work.** What sui-id
  does not protect against, what is on the roadmap, and what
  is explicitly out of scope.

If you find a discrepancy between this document and the source,
the source wins — please open an issue. See the bottom for the
reporting channel for security-sensitive findings.

---

# Part 1 — Foundations

## 1.1 Scope

sui-id is a self-hosted OpenID Connect (OIDC) provider that runs
as a single Rust binary backed by a single encrypted SQLite
database. The threat model in this document covers:

- The HTTP surface (admin UI, OIDC endpoints, OAuth2 token
  flow, RFC 7662 introspection, RFC 7009 revocation,
  WebAuthn / TOTP / step-up, forgot-password, profile, admin
  settings).
- The on-disk surface (SQLite database, master key file,
  backups, rotation files).
- The integration surface with one external service (the
  Pwned Passwords API at `api.pwnedpasswords.com`) and one
  optional external service (the operator-configured SMTP
  relay).

It does not cover:

- The TLS-terminating reverse proxy in front of sui-id
  (nginx / Caddy / Traefik / cloud LB). sui-id assumes the
  proxy terminates TLS correctly, sets `X-Forwarded-For`
  truthfully, and is itself patched.
- The security of relying parties (RPs / clients) that sit on
  the OIDC consumer side. sui-id ships features like PKCE
  enforcement and `redirect_uri` exact-matching that protect
  *correctly written* clients; it cannot save a client that
  echoes its access token in a public URL.
- The host operating system, container runtime, and physical
  hardware. A root-level attacker on the host can read the
  master key file and the DB; this is treated as out of
  scope (Part 5).

## 1.2 Trust boundaries

sui-id has six interfaces with the outside world. Each is a
trust boundary:

1. **HTTPS (public)** — the proxy-to-sui-id and proxy-to-RP
   path. We assume the path is not trustworthy beyond TLS
   confidentiality and integrity guarantees. We do not assume
   the proxy is on the same host (though it usually is).
2. **HTTPS (internal)** — administrative endpoints
   (`/admin/...`). Same TLS assumptions; the additional
   protection is server-side authentication, CSRF, and
   step-up checks (Part 2).
3. **Filesystem** — the SQLite DB file, the master key file,
   backup tarballs, and the rotation `.bak.<ts>` files. We
   assume the host filesystem permission model (Unix DAC) is
   in force; we do not assume disk-encryption is present
   (column encryption is a defence in its absence).
4. **SMTP (out)** — the operator-configured relay used for
   forgot-password / password-changed notifications. Treated
   as untrusted. The credentials sui-id ships to the relay
   are sealed in `smtp_config.password_enc`.
5. **HTTPS (out, fixed)** — the Pwned Passwords API. Treated
   as untrusted with respect to the requesting payload (the
   privacy guarantee is structural — k-anonymity, see 2.9);
   treated as untrusted with respect to the response (a
   compromised HIBP cannot trick sui-id into rejecting a
   legitimate password — the worst it can do is fail-open).
6. **Operator** — a human with shell access to the host. The
   operator is trusted: a malicious operator can read the
   master key, edit the DB, and impersonate any user. Most
   defences in this document do not apply against the
   operator. The audit-log hash chain (3.4) is the one
   exception.

## 1.3 Adversaries

Five adversary classes show up repeatedly in Part 2:

- **N — Network adversary.** Sees encrypted traffic on the
  wire. Has not compromised TLS. Can replay, reorder, drop,
  and (in some scenarios) MitM if they have a valid cert for
  the sui-id origin (e.g. via a stolen CA private key).
- **C — Online attacker.** Connects to sui-id over the public
  HTTPS endpoint as any user might. Submits credentials,
  attempts password / token guessing, attempts
  CSRF / open-redirect / replay.
- **L — Local read-only attacker.** Has obtained a copy of
  the SQLite file (e.g. an unauthorised backup, a snapshot
  read off a compromised replica). Does not have the master
  key file.
- **L+K — Local read-write attacker with master key.** Has
  obtained both the SQLite file AND the master key file. Can
  decrypt every sealed column. The audit hash-chain (3.4)
  detects, but cannot prevent, malicious modification.
- **B — Browser-side attacker.** Controls a user's browser at
  varying levels — CSP-bypassed XSS at the highest end, a
  cross-origin script with no DOM access at the lowest.
  WebAuthn passkeys, cookie scope, and CSRF tokens are
  defences here.

A sixth class — **insider operator** — is mentioned for
completeness but explicitly out of scope (1.1, 5.1).

## 1.4 Asset taxonomy

The defences in Part 2 only make sense once you know what
they're protecting. sui-id holds these classes of secret:

- **Authentication secrets**
  - User passwords (Argon2id-hashed, never stored in
    plaintext; only `credentials.password_hash`).
  - TOTP shared secrets (`user_totp.secret_enc`, sealed).
  - TOTP recovery codes (`user_totp.recovery_codes_enc`,
    sealed).
  - WebAuthn passkey state
    (`user_webauthn_credentials.passkey_enc`, sealed).
- **Session and authorisation state**
  - UI session ids (`sessions.id`, opaque random,
    HttpOnly + SameSite=Lax + Secure-on-config cookie).
  - OAuth2 refresh tokens
    (`refresh_tokens.token_enc`, sealed; the *plaintext*
    token is given to the client once, never re-derivable
    from the DB even with the master key without the
    original).
  - Active access tokens (JWTs; not stored — verified by
    signature against `signing_keys.public_key` + revocation
    list).
- **Identity material**
  - User profiles (username, optional display name, optional
    email, preferred language). Username is plain; email is
    plain (used to send mail).
- **Server-side cryptographic material**
  - JWT signing keys (`signing_keys.private_key_enc`,
    sealed, Ed25519).
  - Master key (32 bytes, lives in the key file; not in the
    DB; XChaCha20-Poly1305 key for column encryption).
- **Operational state**
  - SMTP credentials (`smtp_config.password_enc`, sealed).
  - Audit log entries (`audit_log`; not encrypted, but
    hash-chained — see 3.4).
  - Server settings (UI default language, HIBP mode, idle
    timeout, concurrent-session cap; not secret).
- **Backup material**
  - Encrypted backup tarballs (operator-controlled
    passphrase + Argon2id KDF + XChaCha20-Poly1305 envelope).
  - The `.bak.<timestamp>` archived key file from a previous
    rotation (same secret class as the active master key —
    must be protected the same way).

---

# Part 2 — Threat scenarios

This part walks through twelve concrete scenarios. Each one
follows the same shape:

> **What** the attacker is trying to do.
> **How** they would attempt it.
> **Adversary class.**
> **Defences.**
> **Residual risk** — what the defences do not cover.

## 2.1 Credential stuffing on `/admin/login`

**What.** Try a known-leaked username / password pair against
sui-id, hoping the same password is reused.

**How.** Submit POST `/admin/login` with each candidate.
Iterate over a leaked-credentials wordlist. Modern lists are
hundreds of millions of pairs; success rates against typical
sites without anti-stuffing controls are ~0.1–1 %.

**Adversary class.** C (online).

**Defences.**

- **Per-account lockout.** The third consecutive failure
  starts a backoff window that grows with each subsequent
  failure (capped at the configured `max_lockout`, default
  one day). The window is per-user, not per-IP, so an
  attacker stuffing many usernames still gets locked out on
  each one.
- **Timing-equivalent refusal paths.** Wrong username,
  wrong password, locked-out account, and disabled account
  all return responses that take indistinguishable time.
  The per-user dummy-Argon2 verification on the
  no-such-user path is the load-bearing piece; without it,
  account enumeration by latency would be trivial.
- **Pwned Passwords (HIBP) at password-set time.** New
  passwords run through the k-anonymity API; in `block`
  mode, a pair already known to attackers cannot be set in
  the first place.
- **Audit row** on every failed login (`auth.login.failed`)
  with hash-chained integrity. Operators see attack volume
  even if the per-user lockout absorbs the individual
  attempts.

**Residual risk.** Stuffing a single user with a small list
that fits inside the first two attempts (no lockout yet) is
not blocked. The third attempt is.

## 2.2 Online password / token grinding on `/oauth2/token`

**What.** The OAuth2 token endpoint accepts client credentials
and refresh tokens; an attacker with a leaked refresh token or
a guess at a client_secret might try to enumerate.

**How.** POST `/oauth2/token` with grant_type variations.
Token-grinding here is rate-limited and audited; client_secret
guessing is bounded by the `client_secrets` Argon2 verification
cost.

**Adversary class.** C (online).

**Defences.**

- **Rate limit** keyed on client IP, scoped to the token
  endpoint specifically. The window is short (seconds);
  bursts above the limit return 429.
- **PKCE enforcement** for the authorization-code flow:
  S256-only at v0.18+. An intercepted code without the
  verifier is useless.
- **Refresh-token rotation with theft detection.** Every
  refresh issues a new token and revokes the one used. A
  presented refresh that has already been rotated triggers
  a family-wide revocation: the legitimate session and all
  descendants are killed. The response from sui-id is
  identical whether the token was legitimately rotated or
  replayed by a thief — the audit log is the only place
  that tells you.
- **Argon2 verification** for client secrets, hashed with
  per-client salts.

**Residual risk.** A stolen refresh token used before the
legitimate user uses it again succeeds once; the rotation +
theft-detection only fires when both presentations are seen.

## 2.3 Session hijack via stolen cookie

**What.** An attacker has obtained a user's `sui_id_session`
cookie (browser malware, intercepted log file, screen-recording
malware, etc) and wants to use it as that user.

**How.** Replay the cookie against `/admin/...` or
`/me/security/...` from the attacker's own machine.

**Adversary class.** C (online), B (browser-side).

**Defences.**

- **Step-up freshness** for sensitive actions. Revoking
  other sessions, signing-key rotation, etc, require a
  step-up completed within the last five minutes. A cookie
  alone is not enough.
- **Idle session timeout** (v0.25+). Configurable. When set,
  a cookie that has not been presented within the window
  is revoked on the next request. Bounds the
  post-compromise window for an idle stolen cookie.
- **Concurrent session cap** (v0.25+). Configurable. When
  enabled, the legitimate user's next sign-in evicts the
  oldest existing session; if that's the stolen one,
  attacker access ends without the user knowing they were
  ever compromised.
- **Cookie attributes.** `HttpOnly` (no JS access from a
  cross-origin script), `SameSite=Lax` (no cross-site POSTs
  with credentials), `Secure` when `cookie_secure = true`
  (no plaintext exfiltration if the user visits a
  same-domain non-HTTPS subresource).
- **CSRF token** on every state-changing POST. A stolen
  cookie alone cannot drive a malicious form submission
  from another origin.
- **Audit row** (`auth.session.created` / `_revoked` etc)
  records new-IP sign-ins; correlated review surfaces
  anomalies.

**Residual risk.** Without step-up enabled and with idle
timeout disabled, a stolen cookie is good for the cookie's
absolute expiry. Operators are advised to enable both in the
Settings → Security tab.

## 2.4 Account enumeration via timing or response shape

**What.** Determine whether a username / email exists, without
the right password. Useful as a precursor to 2.1.

**How.** Probe `/admin/login`, `/forgot-password`, and the
admin user-management UI; compare response times and bodies
between known-existing and known-nonexistent identifiers.

**Adversary class.** C.

**Defences.**

- **Login.** Wrong-username and wrong-password paths are
  timing-equivalent (2.1).
- **Forgot-password.** Always returns 200 with the same
  neutral page regardless of whether the supplied address
  matches a known user. The mail is sent (or not) entirely
  on the server side; no observable bit leaks to the
  requester.
- **Admin user search** is gated behind `CurrentAdmin` and
  is not part of the public surface; enumeration via this
  path is by definition a post-authentication concern.

**Residual risk.** Side-channels we have not enumerated may
exist — stack-allocator behaviour, network buffering, etc.
We treat constant-time-equivalent paths as a property to
maintain in code review and in the e2e tests, not as a
guarantee.

## 2.5 CSRF on the admin UI

**What.** Trick a logged-in admin into POSTing a state-changing
request from another origin.

**How.** Host an attacker page that auto-submits a hidden form
to `https://idp.example.com/admin/users/{id}/disable`.

**Adversary class.** B (browser-side).

**Defences.**

- **CSRF token** required on every state-changing POST.
  Token lives in a cookie (`sui_id_csrf`) and is also
  echoed into a hidden form field; the handler refuses if
  the cookie value and the form value diverge or are
  missing.
- **`SameSite=Lax`** on the session cookie. A cross-site
  POST will not include the session cookie at all on most
  browsers, so even without the CSRF token the request
  lands unauthenticated.
- **`Origin` header check** is not used as a primary defence
  in v0.26.0 — the CSRF token + SameSite combination is
  sufficient — but we plan to add it as a belt-and-braces
  cross-check.

**Residual risk.** Browsers without `SameSite=Lax`
implementation (very old, embedded webviews) reduce one of
two layers; the CSRF token still holds.

## 2.6 Stolen DB file (column-encryption defence)

**What.** Attacker has an unauthorised copy of `sui-id.sqlite`
but does not have the master key.

**How.** Off-host backup copy, snapshot of a replica that the
attacker has access to, etc.

**Adversary class.** L (local read-only).

**Defences.**

- **Column encryption** on every secret asset class (1.4):
  signing keys, refresh tokens, TOTP secrets, TOTP recovery
  codes, WebAuthn passkeys, SMTP password.
  XChaCha20-Poly1305 with a per-column AAD that binds the
  ciphertext to its semantic role (a refresh-token
  ciphertext cannot be re-purposed as a signing-key
  ciphertext, even with the same key — open() fails).
- **Master key separation.** The key is not in the SQLite
  file. It lives in the configured `key_file` (or the
  `SUI_ID_MASTER_KEY` env var). An attacker with only the
  DB file gets nothing decryptable.
- **Argon2id** for password hashes. Brute-forcing the
  `credentials.password_hash` column is bounded by the
  cost parameters; default settings are well above the
  CPU-efficient line.
- **SHA-256-of-token storage** for password-reset tokens.
  Even with the master key, the database does not contain
  reset tokens in any form that lets an attacker re-issue
  them — they are pre-image-resistant hashes.

**Residual risk.** The username, email, display name, and
preferred language columns are not encrypted. A stolen DB
file leaks the user list and their language preferences.
This is by design: encrypting these would prevent admin UI
search and locale resolution from working, with little
defensive payoff (an attacker with the DB file already has
the user list as a side-effect of the schema).

## 2.7 Stolen DB + master key (rotation as the response)

**What.** Attacker has both the SQLite file and the master
key. Every sealed column decrypts.

**How.** Compromise of the host filesystem broadly enough to
read both files.

**Adversary class.** L+K.

**Defences.**

- **Master-key rotation** (v0.26+). The CLI re-seals every
  encrypted column under a new 32-byte XChaCha20-Poly1305
  key in a single SQLite transaction. After rotation, the
  pre-rotation key file (which the attacker holds)
  decrypts nothing. The pre-rotation DB file (which the
  attacker also holds) decrypts what was current at
  rotation time but not later.
- **Audit hash chain** (3.4). Even if the attacker writes
  to the DB after compromise, the chain breaks at the first
  unauthorized append; an operator running
  `sui-id verify-backup` or comparing two snapshots over
  time can detect the tampering.
- **Refresh-token rotation + theft detection** (2.2). A
  stolen refresh token presented after the legitimate
  rotation triggers family-wide revocation.

**Residual risk.** Anything the attacker did between
exfiltration and detection is undetectable; the audit log
detects tampering but does not unsend already-issued
tokens. Rotation invalidates future use of stolen material
but does not unwind past use. This is an inherent property
of post-incident response, not a sui-id-specific gap.

## 2.8 Phishing reset-link redemption

**What.** Trick a user into clicking a `/reset-password?token=…`
link that the attacker controls, then take over the account.

**How.** The attacker either (a) generated the link by
requesting a reset for the victim's address (legitimate
request, attacker hopes to intercept the email) or (b) crafted
a fake link that looks legitimate.

**Adversary class.** C plus a phishing channel (email,
chat, etc).

**Defences.**

- **30-minute TTL** on reset tokens.
- **Single-use redemption.** A successful reset marks the
  token consumed; a replay returns the "invalid or expired"
  page indistinguishably from an unknown token (no
  enumeration leak).
- **Hash-only storage.** The token is a 32-byte
  CSPRNG-generated random ID, base64-URL encoded for the
  link, and stored only as `SHA-256(token)` in
  `password_reset_tokens.token_hash`. Even L+K (2.7) does
  not yield reset tokens — only their hashes, which are not
  reversible.
- **At most three outstanding tokens per user.** Older
  tokens are invalidated before the cap kicks in; reduces
  the attack surface of "long-running phishing campaigns
  plus abandoned reset emails".
- **User-enumeration neutral** request flow. The request
  endpoint's response shape is identical for known and
  unknown email addresses (2.4), so an attacker can't
  pivot from "is this email registered" to "request a
  token I can later try to phish".
- **Notification mail** on successful change. The
  legitimate user is notified; if the change wasn't them,
  they have a signal.

**Residual risk.** Email is itself untrusted. A
sufficiently-resourced attacker who can read the user's
inbox can also click the link. The 30-minute TTL bounds the
window.

## 2.9 Pwned Passwords API as adversary

**What.** A compromised or hostile HIBP responds with
attacker-chosen data (or with a faulty response) so as to
poison the password-acceptance decision, or attempts to learn
which passwords sui-id users are setting.

**How.** DNS hijack, BGP hijack, certificate-issuance attack
against `api.pwnedpasswords.com`, or a genuine compromise of
the upstream service.

**Adversary class.** N (network) plus a control plane on the
remote service.

**Defences.**

- **k-anonymity model.** sui-id sends only the first 5
  characters of `SHA1(password)`. The plaintext password is
  never on the wire, never in the request URL, never in
  application logs. A hostile HIBP cannot extract candidate
  passwords; the most it can do is mislead the *acceptance
  decision*.
- **`Add-Padding` header.** Asks HIBP to pad responses to a
  uniform size, defending against traffic-analysis attacks
  that infer the queried prefix from response length.
- **Fail-open.** When the HIBP request fails or returns an
  unparsable body, sui-id treats the result as
  `Unavailable` and proceeds with `HibpEnforcement::Allowed`
  regardless of mode, including `Block`. The audit log
  records the failure. We chose fail-open over fail-closed
  because a flaky external service must not be allowed to
  lock an admin out of password operations.
- **Mode is operator-controlled.** Air-gapped deployments
  set `hibp_mode = 'off'`; no outbound request is made.

**Residual risk.** A hostile HIBP that says "not breached"
when the password *is* breached lets a weak password
through. This is a degradation, not an escalation: in `Off`
mode sui-id makes the same decision unconditionally.

## 2.10 SMTP credential leak via DB exfiltration

**What.** The attacker wants the operator's SMTP relay
credentials to send mail through them (spam, phishing the
operator's customers, blast a targeted message that appears
to come from the operator's domain, etc).

**How.** Stolen DB file (2.6) — the credentials are a
sealed `smtp_config.password_enc` column.

**Adversary class.** L. (For L+K, see 2.7.)

**Defences.**

- **Column encryption** with the dedicated AAD
  `b"smtp.password"`. An attacker with the DB but not the
  master key extracts a ciphertext that does not decrypt.
- **Rotation** (v0.26+). After a suspected DB compromise,
  the operator rotates the master key and the SMTP
  password is re-sealed under the new key. The attacker's
  pre-rotation copy is now useless (2.7).

**Residual risk.** L+K still extracts the SMTP password.
Operators with a high-value SMTP relay should rotate the
password on the relay side after any L+K event.

## 2.11 Backup tarball intercepted in transit

**What.** A backup tarball moves between the host and an
off-site store; an attacker grabs a copy.

**How.** Compromised intermediate hop, misconfigured
permissions on a shared backup target, etc.

**Adversary class.** L (read-only) on the tarball.

**Defences.**

- **Encrypted backup mode** (`sui-id backup --encrypt`).
  Argon2id KDF over an operator passphrase,
  XChaCha20-Poly1305 envelope. Without the passphrase the
  tarball is opaque; with the passphrase the tarball
  reveals the same contents an unencrypted backup would
  (i.e. it includes the master key file by design —
  restore must be one-step).
- **Verify-without-restore** workflow
  (`sui-id verify-backup`). Operators can sanity-check a
  tarball before relying on it, surfacing tampering or
  corruption.
- **Mode-0600 output file** on the host that wrote the
  backup. Doesn't help against in-transit interception,
  but reduces the surface on the source host.

**Residual risk.** A weak passphrase reduces Argon2id's
defence. The tooling does not enforce a minimum-strength
passphrase; the prompt warns but the operator has the
final word.

## 2.12 JWT-signing-key compromise

**What.** Attacker obtains a sui-id signing private key and
forges access tokens, ID tokens, or refresh proofs.

**How.** L+K against the host (2.7), insider operator (out
of scope), exploitation of a memory-disclosure bug
(`unsafe_code = forbid` in the workspace bounds the surface;
unsafe lives only in the audited dependency tree).

**Adversary class.** L+K (most realistic), plus theoretical
memory-disclosure attacks.

**Defences.**

- **Column encryption** of `signing_keys.private_key_enc`
  (2.6) — bounds the L attack class.
- **Master-key rotation** (v0.26+) re-seals signing keys
  under a new master key. After rotation the attacker's
  old signing-key ciphertext is unreadable.
- **JWT `kid` header** identifies the signing key. Rotating
  the *signing key itself* (separate from master-key
  rotation) is supported via the admin UI; tokens issued
  under the old key continue to verify until they expire,
  while new tokens use the new key. Combined with the
  refresh-token rotation, an L+K attacker who steals an
  old signing key still has a bounded forgery window.
- **`alg` allow-list.** sui-id refuses to verify tokens
  signed with anything other than its supported algorithms;
  the classic "alg=none" and "alg=HS256 with RSA pubkey"
  confusions are statically blocked.

**Residual risk.** A stolen *active* signing key is a
forgery oracle until either (a) rotation, or (b) the token
expiry. Tokens are short-lived (minutes by default); the
window is bounded but real.

---

# Part 3 — Defensive properties

These are the architectural invariants that hold across all
features. If a future change would break one of them, that
change needs the threat model updated alongside it.

## 3.1 Master-key separation

The master key is *never* stored in the SQLite file. It lives
either in `SUI_ID_MASTER_KEY` (env) or in `key_file` (FS).
Possession of `sui-id.sqlite` alone — without the key — yields
nothing decryptable. This is the foundational property that
2.6 builds on.

The rotation CLI (v0.26+) preserves this property: the new
key file is written with mode `0600` on Unix, the old file is
renamed to `.bak.<timestamp>` in the same directory, and at
no point is the key written into the DB.

## 3.2 Column-encryption AAD binding

Every sealed column is sealed with a column-specific AAD:

| Column                                         | AAD                                              |
| ---------------------------------------------- | ------------------------------------------------ |
| `signing_keys.private_key_enc`                 | `sui-id/signing_key/v1`                          |
| `refresh_tokens.token_enc`                     | `sui-id/refresh_token/v1`                        |
| `user_totp.secret_enc`                         | `sui-id/user_totp/v1`                            |
| `user_totp.recovery_codes_enc`                 | `sui-id/user_totp/recovery/v1`                   |
| `user_webauthn_credentials.passkey_enc`        | `sui-id/user_webauthn_credentials/passkey/v1`    |
| `smtp_config.password_enc`                     | `smtp.password`                                  |

XChaCha20-Poly1305 binds the ciphertext to its AAD. Moving a
ciphertext to the wrong column (e.g. UPDATE-injecting a
refresh-token ciphertext into `signing_keys.private_key_enc`)
fails to decrypt at use time, even with the master key. This
makes an L+K attacker's job harder than "decrypt one thing,
substitute it everywhere".

## 3.3 Refresh-token theft detection

The refresh-token store is built around a *family*: each
issued refresh ties to its issuance ancestor. Rotation creates
a new token in the same family and revokes the previous one.
A presented refresh that has already been rotated triggers
*family-wide* revocation: the entire chain dies, not just the
replayed token.

Two important properties hold:

- **Indistinguishable response.** sui-id's response to the
  attacker (or to the legitimate user, depending on which
  party gets there first) is the same shape regardless of
  whether the rotation was legitimate or adversarial. The
  audit log is the only place that records the
  family-revoke.
- **PKCE enforcement.** S256-only at the authorization-code
  flow. An intercepted code without the verifier cannot be
  redeemed — defence in depth on top of the rotation logic.

## 3.4 Audit-log hash chain

Every audit row's `hash` column is
`SHA-256(prev_hash_hex || length-prefixed canonical bytes)`.
A row's bytes are deterministic over the row's content
(actor, action, target, result, timestamp, note). Inserting
or modifying a row breaks the chain at that row and at every
row after it.

Properties:

- **Tamper-evident, not tamper-proof.** A determined L+K
  attacker can rewrite the chain end-to-end (every row's
  hash recomputed). What they cannot do is *partially*
  rewrite — once they touch one row, they have to fix every
  subsequent row, which means they need the
  column-encryption state of those rows to be self-
  consistent and the timestamp ordering to remain monotone.
- **Detection via off-host comparison.** An operator who
  exports audit rows nightly and compares against the live
  DB sees the divergence at the first row touched. The
  `verify-backup` CLI runs the same check against backup
  contents.
- **Operator out of scope.** A malicious operator with
  shell access can rewrite anything; the chain doesn't
  claim to defend against insiders.

## 3.5 Step-up freshness

Five-minute window. A session that has just signed in gets a
fresh `last_step_up_at`; a session that has merely been
*present* (touched, see 3.7) does not. Sensitive actions
(revoke other sessions, signing-key rotation, irreversible
admin operations, etc) check the freshness window and refuse
otherwise.

Two non-obvious choices:

- **Password change is *not* gated** behind step-up. The
  user must enter the current password to change it; that
  is itself a fresh proof and gating it twice would
  cascade-prompt for no defensive benefit.
- **WebAuthn step-up uses `kind = 'step_up'`** in the
  `webauthn_pending` table; finishing with a `kind` other
  than `step_up`, or with a `user_id` other than the one
  in the pending row, refuses *without* consuming the row,
  so a CSRFed completion attempt cannot waste a legitimate
  user's pending challenge.

## 3.6 User-enumeration neutrality

`/forgot-password`, the login endpoint, and the admin
user-search endpoint follow the same neutrality principles:

- Identical response shape between "exists" and "doesn't
  exist".
- Timing-equivalent paths (with dummy Argon2 verifications
  on the no-such-user branch).
- Audit rows are created for both branches with action names
  that distinguish them, but those rows are not on the
  request response path.

## 3.7 Idle-timeout throttle

The idle-timeout feature requires a "most recent presentation"
timestamp; writing it on every authenticated request is
wasteful. sui-id throttles writes: `sessions.last_used_at` is
updated only when its current value is more than 60 seconds
old. Properties:

- **Bounded write rate.** A busy session generates roughly
  one write per minute, not one per request.
- **Timeout granularity preserved.** A few-minute idle
  timeout still reflects actual usage; a 10-second timeout
  would be quantised to 60 seconds, but operationally
  setting an idle timeout below the throttle window is
  wrong on its own.
- **Pre-migration sessions.** Rows created before the
  v0.25.0 migration have `last_used_at = NULL`; the
  resolver treats `NULL` as "as old as `created_at`", so
  they fall under the same idle policy as a brand-new
  session that has not yet been re-presented.

## 3.8 `unsafe_code = forbid` at the workspace level

The Cargo workspace pins `unsafe_code = "forbid"`. sui-id's
own crates have zero `unsafe` blocks. Every `unsafe` lives in
audited dependencies (the cryptography stack:
`chacha20poly1305`, `x25519-dalek`, `argon2`, `rusqlite`'s C
bindings).

This is a property, not a guarantee — a sufficiently subtle
soundness bug in a `safe` API could still cause memory-safety
issues — but it removes a large class of mistakes from
sui-id-authored code.

---

# Part 4 — Detailed concerns

This part is for security auditors and enterprise reviewers
doing a thorough pass. It re-frames Parts 2 and 3 against the
STRIDE taxonomy, sketches attack-tree fragments for the most
common questions, and gives compliance-relevant pointers.

Operators can skip this part on first read.

## 4.1 STRIDE summary

STRIDE classifies threats by intent. For each category, the
most relevant scenarios from Part 2 and the architectural
properties from Part 3 are listed.

### Spoofing

Threats: an attacker impersonates a legitimate user, an admin,
sui-id itself, an OIDC client, or the HIBP service.

- 2.1 (credential stuffing) — defended by lockout, timing
  equivalence, HIBP at password set.
- 2.3 (session hijack) — defended by step-up, idle timeout,
  cap, cookie attributes, CSRF.
- 2.9 (HIBP impersonation) — bounded by k-anonymity (2.9
  defences) and by fail-open semantics.
- Client spoofing — `client_secret` Argon2 verification +
  PKCE for public clients + exact-match `redirect_uri`.
- Self-spoofing — TLS terminates at the proxy; sui-id
  assumes the proxy is correctly configured. Without TLS
  the attacker on the path can substitute responses freely.

### Tampering

Threats: modify data in transit, in storage, or in audit
records.

- TLS confidentiality + integrity (assumed) covers in
  transit.
- 2.6 + 2.7 cover the DB at rest (column encryption +
  master-key separation + rotation).
- 3.4 (audit hash chain) covers append-only-ness and
  tamper detection.
- HTTP-layer tampering on POSTs is bounded by CSRF tokens
  (2.5) and `SameSite=Lax`.

### Repudiation

Threats: a user (or operator) denies having taken an action.

- Audit log captures actor, action, target, result, and
  timestamp for every state-changing operation.
- Hash chain (3.4) makes tampering with the trail
  detectable.
- Insider operators can still tamper given shell access;
  the log is tamper-evident, not tamper-proof, against this
  adversary.

### Information disclosure

Threats: leak data that should be confidential.

- 2.4 (account enumeration) — defended by user-enumeration
  neutrality (3.6).
- 2.6 (stolen DB) — defended by column encryption (3.2) and
  Argon2id password hashing.
- 2.10 (SMTP password leak) — defended by column
  encryption.
- 2.11 (backup interception) — defended by `--encrypt`
  mode.
- HIBP — sui-id's own request payload reveals only a 5-char
  hash prefix, structurally limited (2.9).
- Web-side surface: `Referer-Policy: same-origin`, no
  password in query strings, and `Cache-Control: no-store`
  on authenticated pages keep secrets out of HTTP caches
  and third-party referrer logs.

### Denial of service

Threats: prevent legitimate use.

- Per-account login lockout and per-IP rate limits prevent
  resource exhaustion via brute-force loops.
- The OIDC token endpoint and the forgot-password endpoint
  carry their own rate limits.
- HIBP fail-open (2.9) ensures a flaky external service
  does not lock admins out of password operations.
- DB-level: SQLite's WAL mode and a single-writer model
  bounds DB-side contention; sui-id is not designed for
  thousands of concurrent writers.
- We do not have explicit defence against application-level
  resource exhaustion (e.g. an attacker uploading a 10 GB
  multipart body); operators are advised to rely on the
  reverse proxy's body-size limits.

### Elevation of privilege

Threats: a regular user becomes an admin, or an
unauthenticated attacker becomes any user.

- Role gate is `users.is_admin`, checked at every admin
  route via the `CurrentAdmin` extractor. The extractor
  refuses for `is_admin = false`, `is_disabled = true`, or
  `is_deleted = true`.
- Step-up freshness (3.5) gates the most dangerous admin
  operations (revoke-others, signing-key rotation).
- Open-redirect / `redirect_uri` confusion is blocked by
  exact-string matching against `clients.redirect_uris`.

## 4.2 Attack-tree fragment: account takeover

A composed attack-tree for "attacker takes over a target user
account":

```
Take over user X
├── Acquire credentials directly
│   ├── Phish password from user                  (2.1, 2.8)
│   │   ├── Phishing site                         → user judgment
│   │   └── Attacker-controlled reset link        → 2.8
│   ├── Stuff leaked credentials                  → 2.1
│   ├── Brute-force online                        → 2.1 (lockout)
│   ├── Crack password from stolen DB             → 2.6 (Argon2)
│   └── Crack password from L+K                   → 2.7 (Argon2 + rotation)
├── Replay an active session
│   ├── Steal session cookie                      → 2.3
│   ├── Steal refresh token                       → 2.2 (theft detection)
│   └── Steal access token                        → bounded by short TTL
├── Forge a token
│   ├── Steal active signing key                  → 2.12 (rotation)
│   ├── alg=none confusion                        → 4.1 (allow-list)
│   └── HMAC-vs-RSA key confusion                 → 4.1 (allow-list)
└── Bypass MFA
    ├── Phish TOTP                                → 2.8 (phishing channel)
    ├── Steal recovery codes                      → 2.6 (column encryption)
    ├── Steal WebAuthn passkey state              → 2.6 (column encryption)
    └── Coerce step-up bypass via CSRF            → 2.5 + 3.5 kind binding
```

Each leaf maps to a Part 2 scenario or a Part 3 property.
Attack paths that are not in this tree (e.g. social
engineering of the operator) sit at adversaries we don't
plan for (5.1).

## 4.3 Compliance hints

This section maps sui-id's defences onto the most commonly
asked-about compliance frameworks. It is not certification
advice and does not create any warranty.

### GDPR / personal data

- **Data minimisation.** sui-id stores username (mandatory),
  email (optional), display name (optional), preferred
  language (optional). The DB schema does not include
  fields beyond what's needed for OIDC.
- **Encryption at rest.** Sensitive columns are sealed
  (3.2); passwords are Argon2id-hashed.
- **Right to erasure.** `users.is_deleted = 1` soft-deletes;
  hard-delete is supported via direct DB operation by an
  operator. (No CLI for this in v0.26.0; see ROADMAP for
  the "right-to-erasure CLI" entry under future work.)
- **Audit log.** Records who did what, when, with
  hash-chain integrity (3.4) — the trail expected for a
  "data controller's record of processing".

### SOC 2 type II / ISO 27001

- **Access controls.** `users.is_admin` + step-up freshness
  (3.5) enforce least-privilege for admin operations.
- **Audit and change tracking.** Hash-chained audit log
  (3.4) covers record-of-change requirements.
- **Cryptographic key management.** Master-key rotation
  (v0.26+) addresses the "key rotation" control. The
  rotation CLI archives the old key (2.7 + 3.1) so that
  pre-rotation backups are still recoverable.
- **Backup integrity.** `verify-backup` CLI provides
  on-demand integrity check; encrypted-mode backups
  address the "backups protected to the same level as
  production data" requirement.

### Other

- **NIST 800-63B password guidance.** sui-id implements the
  recommended block-list approach via HIBP integration
  (2.9). Configurable mode (`off / warn / block`) lets
  operators match their authority's specific stance.
- **OWASP ASVS V2 (authentication)** — most level 2
  controls are in place: lockout, MFA, session lifetime,
  secure cookie attributes, password rotation hooks.

## 4.4 What auditors typically ask

A short FAQ. Source-level evidence is in
`docs/operators.md` and the per-feature CHANGELOG entries.

> *Q: What random-number source does sui-id use?*
>
> All security-relevant randomness comes from `OsRng`
> (`getrandom` under the hood). This includes session IDs,
> reset tokens, refresh tokens, master-key generation, and
> nonces for column encryption. PRNG-only randomness is
> never used for security purposes.

> *Q: How are passwords hashed?*
>
> Argon2id with parameters tuned for the host. Default
> parameters are above the OWASP-recommended floor;
> operators can override in the config file. Per-user
> salts.

> *Q: What HTTPS / TLS profile does sui-id support?*
>
> sui-id does not terminate TLS itself in v0.26.0;
> deployment is behind a reverse proxy. The proxy's TLS
> configuration is the operator's responsibility. We do
> not mandate a profile.

> *Q: How long is a session valid?*
>
> Configurable. The cookie carries a server-side opaque
> session ID; the row in `sessions` has an `expires_at`
> (default 7 days at v0.26.0) and an optional
> `idle_session_timeout_secs` (off by default; set in
> Settings → Security to enable). Both are checked at
> every authenticated request.

> *Q: Is MFA mandatory?*
>
> Not by default. Admins can require MFA per-user via the
> profile UI. Step-up MFA (5-minute freshness window) gates
> the most dangerous admin operations regardless of whether
> a user has MFA enrolled (3.5).

> *Q: How does sui-id integrate with HSMs?*
>
> The `--new-key PATH` option to `admin rotate-key`
> accepts a key file produced anywhere, including by an
> HSM-fronted workflow. The runtime cryptography is in-
> process: a PKCS#11 backend is on the longer-term
> roadmap and is not present in v0.26.0.

---

# Part 5 — Known limitations and future work

## 5.1 Adversaries we don't plan for

- **Operator with host shell access.** Anyone who can `cat
  master.key` and read the DB file is treated as the
  trustworthy administrator. The audit hash chain (3.4)
  detects log tampering but does not prevent it; column
  encryption is built around master-key separation, which
  is irrelevant once the operator has both files.
- **Memory-disclosure attacks against the running process.**
  We rely on `unsafe_code = forbid` in our own crates and
  on the audited cryptography stack for the rest. We do
  not use `mlock` or hardware memory-tagging; a
  kernel-level attacker reading process memory wins.
- **Physical access** to the host. Disk encryption (LUKS,
  FileVault, etc) is the operator's responsibility.
- **Supply-chain compromise** of our crates' dependencies.
  We pin versions, run `cargo audit` in CI, and review
  changes during dependency bumps. We do not have
  reproducible-build infrastructure.

## 5.2 Limitations we intend to fix

See `ROADMAP.md` for the live list. As of v0.26.0 the most
relevant items are:

- **Hot master-key rotation.** The current CLI is offline.
  The complexity ladder is steep enough that we
  deliberately rejected it for v0.26.0; this section
  captures the decision rather than a planned change.
- **HIBP scope expansion.** v0.24.0 wires the breach check
  only into the setup wizard. The remaining password-set
  entry points (self-service password change, admin
  reset, forgot-password redemption) are mechanical to add
  now that the trait + policy function are in place; they
  appear in the ROADMAP "HIBP scope expansion" entry.
- **Periodic password re-check.** A "your last sign-in's
  password is now in a breach" notification on next
  sign-in needs a careful privacy story (see ROADMAP).
- **Right-to-erasure CLI.** Currently soft-delete is admin
  UI; hard-delete is a manual DB operation. A
  `sui-id admin delete-user --hard` is on the roadmap.

## 5.3 Things explicitly out of scope

- **SAML.** sui-id is OIDC-only.
- **Implicit and hybrid OIDC flows.** PKCE-protected
  authorization-code is the only flow we support.
- **OPAQUE / aPAKE password upgrade.** Argon2id over the
  TLS channel is the in-scope password verification.
- **External user-store backends beyond the current LDAP shim.**
  sui-id supports a read-only LDAP authentication source with
  local shadow rows. Broader directory ownership, write-back, and
  AD-specific semantics remain out of scope.
- **Multi-tenancy.** Every client and every user share one
  flat namespace.

---

# Reporting security issues

If you find a security issue in sui-id, **please do not file
a public GitHub issue**. Mail nabbisen <nabbisen@scqr.net>
with a description, ideally including:

- the version of sui-id you reproduced against,
- a minimal reproducer or PoC, and
- whether you'd like to be credited in the eventual
  CHANGELOG entry.

We aim to acknowledge within 48 hours and to issue a fix or a
mitigation guide within two weeks for high-severity
findings. Lower-severity findings are folded into the regular
release cycle.
