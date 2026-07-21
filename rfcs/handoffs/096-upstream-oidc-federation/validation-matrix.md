# RFC 096 normative validation matrix

All rejection results are fixed local errors with no attacker-controlled echo.
“Reject” means no mapping/session authority and, after callback claim, no
attempt retry.

## Configuration and URLs

| Input | Accept | Reject |
|---|---|---|
| Issuer | Canonical HTTPS hostname URL, optional path | HTTP; IP literal; userinfo/query/fragment; noncanonical; local/internal suffix |
| Endpoint | Canonical HTTPS; origin and port in provider policy | Discovery-added origin; redirect; downgrade; fragment/userinfo |
| Origins | 1–8 explicit canonical origins including issuer | wildcard, suffix rule, path, inferred redirect target |
| Port | Explicitly allowed 1–65535; default 443 | zero, overflow, unlisted, implicit alias mismatch |
| Scopes | unique subset `openid email profile`, includes `openid` | missing openid, unknown/duplicate/oversize |
| Token auth | `client_secret_basic` with secret; `none` without | POST, mixed/missing secret, other method |
| ID-token algs | nonempty subset RS256/PS256/ES256/EdDSA | none, HS*, unknown, empty |

Canonical corpus includes scheme/host case, default ports, trailing dots,
Unicode/punycode aliases, dot segments, encoded separators, backslashes,
empty hosts, IPv6 literals, credentials, fragments, and malformed percent
escapes. Tests assert the exact accept/reject value, not merely parser success.

## DNS, TLS, HTTP, and JSON

| Boundary | Accept | Reject |
|---|---|---|
| DNS answers | 1–8; every normalized address ordinary public unicast outside all IANA-special and explicit deny prefixes | empty/over-limit; any IANA special row regardless of global flag; NAT64/6to4/Teredo/mapped/compatible/anycast/other explicit deny |
| Connection | dial one validated address; original-host SNI/cert; pinned rustls/TLS 1.2–1.3; handshake ≤256 KiB inbound, chain ≤16/128 KiB | second DNS, host mismatch, invalid cert, TLS <1.2, handshake/message/chain/configuration over bound |
| HTTP version/allocation | HTTP/1.1 ALPN only; fixed 32 KiB header buffer + 64 slots + 8 KiB scratch | HTTP/2/1.0/upgrade/informational chain; grow/retry parser allocation |
| Redirect | none | every 3xx regardless of same origin/scheme |
| Proxy/ambient state | none | environment proxy, cookie jar, inherited auth/client cert |
| Timing | connect ≤3s, headers ≤5s, total ≤10s | slow connect/header/body past limit |
| Header envelope | status ≤1 KiB; ≤64 fields in fixed parser; semantic total ≤32 KiB, name ≤64, value ≤8 KiB; ETag ≤256 | bare LF/obs-fold, invalid/control, duplicate singleton, incomplete/excess before response construction |
| Body framing | one canonical length, sole chunked, or connection-close; fixed scratch; empty trailer section; declared length read exactly then one-use connection dropped without EOF wait | duplicate/conflicting length, length+TE, stacked/unknown TE, chunk extension/line >128, every trailer |
| Media/status | 200 JSON/+json; conditional 304 only under cache rules | every other status; wrong/absent 200 media type |
| Body | endpoint byte cap, depth/member/string/array caps | compressed encoding, over-limit, duplicate key at any depth |

Resolver tests include both edges of every vendored IANA/explicit prefix,
ordinary public IPv4/IPv6 peers, mixed public/private answers, rebinding,
address-order changes, IPv4-mapped/compatible, IPv6 zone IDs, metadata-service
addresses, both boundaries of both NAT64 prefixes with public/private/
link-local/metadata embedded IPv4, 6to4, and Teredo. Every NAT64 vector is a
denial because M4 rejects the translation prefixes wholesale.

## Discovery

| Condition | Result |
|---|---|
| issuer missing or not byte-exact | Reject document |
| auth/token/JWKS missing, noncanonical, or origin/port unapproved | Reject document |
| `code`, authorization-code grant, S256, response `iss`, configured token auth, public subjects, or configured algorithm compatibility absent | Reject document |
| bounded unknown metadata | Ignore after envelope accounting |
| endpoint path/query unusual but canonical and origin approved | Accept |

Well-known vectors require root issuer `https://id.example` to derive
`https://id.example/.well-known/openid-configuration` and path issuer
`https://id.example/tenant/a` to derive
`https://id.example/tenant/a/.well-known/openid-configuration`. The
insert-before-path RFC 8414 form is rejected for the latter.

## Preflight and provider activation

| Condition | Result |
|---|---|
| Valid production discovery/JWKS preflight while exact provider is disabled before/after validation | Return sealed in-memory provider/version/generation/policy/time/fingerprint capability; no write |
| Preflight while enabled or enabled during final recheck | No capability/evidence |
| Failed/cancelled preflight or restart | No capability/evidence; rerun required |
| C17 enable with exact disabled provider/version/generation/policy and age 0–599.999s | Atomically generation++ + store evidence bound to new generation + enable + event |
| Age exactly/over 600s, clock regression, wrong/replayed provider/version/generation/policy, already enabled/deleted | Roll back; no enable/event |
| Concurrent distinct same-generation capabilities | Exactly one C17 winner; winner invalidates every sibling |
| C17 disable | Require enabled; atomically generation++ + disable + clear evidence + invalidate flows + event |
| C23 policy replace / C18 delete | Atomically generation++ (and C23 version++) + clear evidence/invalidate flows; old capabilities fail |
| malformed/noncanonical/missing/max generation | Fail closed without mutation/event |
| C16 after any C18 deletion | New globally fresh provider ID; prior ID is never reused |
| Startup with unchanged / changed client secret | Preserve exact existing envelope and counters / reseal and use C23 |

## Callback and attempt

| Condition | Result |
|---|---|
| exact slug, issuer, state, browser binding, version, pending/unexpired | One atomic winner becomes exchanging |
| two simultaneous matching callbacks | Exactly one winner; other generic reject; one token request |
| wrong/missing/duplicate state or `iss`; mixed code/error | Reject before token network |
| provider disabled/version changed after start | Reject; no token network |
| upstream error with exact binding | Consume to failed; no token network |
| failure/cancellation after claim | Terminal/non-retryable; new start required |
| legacy shared callback | Generic local rejection; no exchange |
| cookie copy to another browser | Browser-binding reject |

Property tests cover state/nonce/verifier/binding independence, base64url
length/alphabet, digest comparison, 600-second exclusive expiry, clock
regression, sealed-verifier AAD substitution, cleanup, and invalid `next` paths
including `//`, backslash, schemes, controls, fragments, and encoded ambiguity.

## Token response and JOSE header/key

| Condition | Accept | Reject |
|---|---|---|
| Token response | mandatory ID token ≤16 KiB; bounded ignored tokens | missing/duplicate/oversize ID token; malformed JSON |
| Serialization | compact JWS, exactly 3 segments | JWE, JSON JWS, detached/empty/unencoded payload |
| `alg` | configured/discovered asymmetric intersection | none, HS*, mismatch, absent/unknown |
| `kid` | required visible ASCII 1–128, one compatible key | absent, duplicate, unknown after bounded refresh, ambiguous |
| Other headers | absent `cty`; absent or JWT `typ` | jku/x5u/jwk/x5c/crit/b64; duplicate header |
| RSA | public signing key ≥2048 bits, valid exponent | private members, weak/modulus/exponent mismatch |
| EC | P-256 public signing key for ES256 | wrong curve, invalid point/private member |
| OKP | Ed25519 public signing key for EdDSA | wrong curve/private member |
| JWK policy | compatible alg/use/key_ops | encryption-only, alg mismatch, no verify when key_ops supplied |
| Signature | valid over original protected header/payload | altered segment, wrong key, malleable/invalid signature |

## Claims

| Claim case | Result |
|---|---|
| exact issuer, client in audience, correct azp rule | Accept that dimension |
| wrong/missing issuer/audience; duplicate audience; >8 audiences | Reject |
| multi-audience without exact azp; any present wrong azp | Reject |
| `now < exp + 60`; attempt-bound iat; optional nbf within +60 | Accept time dimension |
| missing/noninteger/overflow exp or iat; too old/future iat; future nbf | Reject |
| mandatory exact nonce | Accept |
| missing, wrong, duplicate, wrong type nonce | Reject |
| sub 1–255 bytes/no control | Accept stable mapping value |
| missing/empty/oversize/control/non-string sub | Reject |
| valid email plus boolean true | Provision-authoritative only |
| absent/invalid email, absent/false/wrong-type verified flag | Never provision-authoritative |
| valid bounded preferred_username/name | Ignore for authority; bounded hint only |
| valid amr ≤16×64, acr ≤256, integer auth_time | Ignore for local MFA authority and persistence |
| malformed/wrong-type/duplicate/oversize optional claim | Reject token deterministically |
| well-formed at_hash ≤256 | Ignore because access token has no authority/use |
| malformed/wrong-type/oversize at_hash | Reject token |

Boundary clocks test exactly `exp+60`, `created-60`, `now+60`, leap-independent
integer arithmetic, and clock regression. Test claims include duplicate keys,
string/floating/exponential NumericDates, Unicode/control subjects, and
single/multiple audience shapes.

## Mapping, MFA, and durable effects

| Case | Result |
|---|---|
| Existing `(provider, sub)` link, active user | Continue to local MFA/session; email not lookup authority |
| Existing link, absent email | Continue; do not erase trusted local email |
| Unknown identity + matching local email | Generic takeover denial; no link/session |
| Unknown provision mode + verified unique email | F04 atomically completes attempt + creates user/link/[Fed] session/cap/event; no MFA branch |
| Unknown provision mode + absent/unverified email | Generic denial; no held/user/link row |
| Unknown link-only identity | Generic link-required denial; no pending-link cookie/row/link |
| User has local MFA | Mandatory local challenge preserving Fed primary |
| Upstream claims MFA, user lacks local MFA | Fed-only ACR1; no local step-up |
| Local TOTP/recovery/WebAuthn succeeds | exact Fed + local method; never Pwd substitution |
| Existing link, no MFA | F01 one Protocol transaction; Class-B success after commit |
| Existing link, MFA required/completed | F02 creates five-minute/count-0 pending; F03 one guarded outcome transaction; no premature success |
| Provider/version/generation/link/user/factor drifts during MFA | F03 Invalidated consumes pending only; no session/link observation/success |
| Browser/CSRF/pending binding fails before MFA verification | Generic unbound denial; no candidate, row touch, or reason selection |
| Bound malformed/wrong/method-substituted input | Private verifier alone returns sealed `BoundRejected` with closed reason; handler cannot construct it |
| Bound wrong/malformed/substituted MFA #1–4 | F03 increments once and returns RejectedStillPending; no anti-replay/session/link observation |
| Bound wrong MFA #5 exactly | F03 increments to 5, exhausts/destroys continuation + ceremonies; no sixth transition |
| Correct MFA at count 0–4 / 5 | Promotes atomically / rejects terminal row |
| Restart/method switch/concurrent wrong+correct | Count persists/shared; serialized guard decides exact winner/boundary |
| F06 WebAuthn start/replacement | One parent/provider/link/user/RP/origin/challenge-bound ceremony, expiry ≤ pending |
| Wrong/correct WebAuthn | Wrong consumes failed ceremony + one count only; correct counter/ceremony/pending commit only with session |
| Wrong TOTP/recovery | Count only; no TOTP step or recovery hash consumption |
| Terminal validation/mapping denial | F05 attempt failure + one exact closed Class-B event/reason; no authority mutation |
| C23/F04 audit append or mutation fails | All owning Class-A effects roll back together |
| F01–F03/F05/F06 protocol write fails | Entire owning compound Protocol transaction rolls back; no false success observation |

## Cache and telemetry

| Case | Required observation |
|---|---|
| Fresh discovery/JWKS | Reuse only exact enabled provider/version/activation generation before monotonic expiry |
| Expired + network failure | Reject; no stale acceptance |
| Unknown-kid storm | one provider-wide refresh/60s, one flight, bounded memory |
| Refresh removes old key | old key no longer accepted from refreshed entry |
| Provider version/disable/re-enable | attempts unusable and an older-generation cache cannot authorize |
| Failure storm | 30s document/provider cooldown, never TTL extension |
| no-store / no-cache / max-age=0 | current validated use only / retained but immediate revalidation / zero freshness; never raised |
| max-age positive/absent | exact positive up to upper cap / default; Age+Date+resident time deducted |
| malformed/duplicate known freshness field | reject response; do not extend old entry |
| 200 with unchanged ETag | fully parse/validate body before replacement |
| 304 | exact conditional ETag + retained cacheable validated same-version/generation body only; present directives replace, absent Cache-Control inherits |
| unknown kid + flight/cooldown/window | join sole flight / fail without budget / fail without network; forced budget starts at dispatch |
| Logs/metrics/audit | closed phase/reason/internal IDs only; forbidden fields absent |

Forbidden-field scanning includes raw/encoded fragments of secret, code, state,
nonce, verifier, token, URL/query, DNS answer, subject, email, JSON body, and
upstream error strings across success and every injected failure.
