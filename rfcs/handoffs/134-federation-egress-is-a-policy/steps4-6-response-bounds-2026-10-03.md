# RFC 134 steps 4 and 6 — D5 Tier 1 (measured bounds) and Tier 3 (the amendment)

**RFC status: Accepted.** Steps 1, 2 and 3 have landed (`981b228`, `842b75b`,
`2425374`).

**Dispatched: steps 4 and 6.** Together they are the last dispatchable work in
RFC 134. They are independent of each other; do them in either order.

**Not in scope: the certificate-chain row.** Its tier is still open and is the
owner's. Nothing here touches it, and step 6 covers **exactly three** rows.

## Facts I verified so you do not have to

**1. Three response bodies are read with no size cap at all.**
`federation.rs:159` (discovery), `:466` (token response), `:878` (userinfo) each
call `.json()`, which reads the whole body into memory unbounded. There is no
`MAX_BODY`, no `take(`, no `content_length` pre-check anywhere on this path.

**2. Two of the matrix's four JSON caps already hold, and should be evidenced
rather than built.**

- **Depth.** `serde_json` 1.0.150 sets `remaining_depth: 128` with
  `disable_recursion_limit: false` (`src/de.rs:34,38,63,67`). Depth is bounded.
- **Duplicate keys.** `serde`'s derived `Deserialize` calls
  `Error::duplicate_field` (`serde-1.0.196/src/de/mod.rs:293`). All three
  federation response types — `RawDiscovery`, `TokenResponse`, `IdTokenClaims` —
  are derived structs; **none deserializes to `serde_json::Value`**, where
  last-key-wins would apply silently. I checked.

**So build the byte cap, the member/string/array caps, and the media/status
checks. For depth and duplicate keys, pin the floor and record what is relied
on** — the Tier 2 pattern from step 1, applied inside Tier 1.

One limit to state in the code rather than discover later: duplicate **unknown**
keys are not caught, because serde skips unknown fields without duplicate
detection. `deny_unknown_fields` is deliberately **not** the fix — a discovery
document legitimately carries many members we do not model, and rejecting them
would break real providers.

## Step 4 — D5 Tier 1: bounds derived from a measured corpus

### 4a — Measure first, then choose the numbers

Fetch the discovery document, JWKS and a userinfo response shape from a corpus
of real providers — **Google, Microsoft Entra, Okta, Auth0, Keycloak, GitLab,
Authentik**. Record observed maxima for body size, member count, string length
and array length.

**Set each bound at observed maximum × a stated headroom**, and record **the
corpus, the date and each number's derivation beside the constant**. You fetched
and cross-checked the IANA registries for D2; the same standard applies — say
which providers you reached, and if one is unreachable say so rather than
quietly dropping it from the corpus.

**A bound nobody can justify is the one that gets raised the first time
something legitimate trips it.** That is the whole reason this step exists
instead of picking round numbers.

### 4b — Enforce them

- **Byte cap, before deserialization.** Replace the three `.json()` calls with a
  bounded read. A `Content-Length` pre-check is necessary but **not sufficient**
  — it is attacker-supplied and may be absent or lie, so the cap must also hold
  on bytes actually read.
- **Media type**: `application/json` or a `+json` suffix. Anything else is a
  rejection, including a 200 with no content type.
- **Status**: 200, or 304 under the cache rules.
- **Member, string and array caps** at the measured bounds.

Rejections take the existing `FetchDiscoveryError::Network` surface — the fetch
failed, from the caller's position. No new user-visible error class, and no
response content in front of the browser.

### 4c — Tests

Each bound gets a test that fails when the bound is removed — the removal check,
as before. For the byte cap, include a body that **lies about its
`Content-Length`**, since that is the case the pre-check alone would miss.

For depth and duplicate keys, a test asserting the *library's* behaviour
(a 200-deep document and a duplicate declared key are both rejected) so the
guarantee is pinned by our suite, not merely inherited.

## Step 6 — D5 Tier 3: the amendment, three rows

`@nabbisen` approved "amend" on 2026-10-03 for exactly three rows:

- the fixed 32 KiB header buffer, 64 slots, 8 KiB scratch;
- the handshake ≤256 KiB inbound bound;
- "declared length read exactly then one-use connection dropped without EOF
  wait".

The RFC already records the amendment and `validation-matrix.md` already carries
the supersession pointer. **What remains is the evidence**, and D5 is explicit
that it must be *evidenced, not asserted*:

- **Pin a dependency floor** for `hyper` and `rustls` in the workspace manifest,
  at versions whose behaviour you have checked.
- **Record what is relied upon**, per row: the upstream behaviour, where you
  verified it, and the version you verified against.
- **If a guarantee turns out to be absent, stop and report it.** D5 says that row
  returns as real work rather than being waved through, and that is a decision
  for me, not a gap for you to close quietly.

Expect this to be small. It is a manifest change and a documented statement; if
it grows into code, something is wrong with my reading and I want to hear it.

## Return

Per-hunk SHA-256 against a stated baseline, the corpus with its date and each
number's derivation, the removal evidence, the per-row verification for step 6,
and the full local gate set — **including A3.2, G19 and G20**.
