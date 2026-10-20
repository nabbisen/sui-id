# M3 — the remaining stage plan, and what `0.83.0` contains

**Written.** 2026-10-20 JST, by the architect.
**Authorized.** `0.83.0` is M3's remaining work — accepted 2026-10-20.
**Governing RFC.** [RFC 095](../../accepted/095-dynamic-client-registration-transaction.md),
**Accepted**; entry gate **open** since 2026-10-05, six of six conditions met.
**Normative source.** [`metadata-validation.md`](metadata-validation.md) — 127
lines, seven sections. This plan maps every remaining one to a stage.
**Landed already.** Stage 1 `78a2dec`, stage 1b `99f83bb`.

## A — one divergence from what was accepted, stated up front

It was accepted as *"M3 stages 2 and 3"*. **This plan proposes five stages,
not two.** The scope is identical — the same five unbuilt sections of
`metadata-validation.md`, nothing added — but the decomposition is finer, and
that is a change worth naming rather than discovering in a dispatch.

**Why, and why it is not ceremony.** "Stages 2 and 3" was a label written in
2026-10-05 before anyone measured what was left. Measured now, the remainder
spans four different file sets and four different kinds of verification: pure
validation functions, an HTTP response surface, token-endpoint client
authentication, and a browser-network observation. Folding them into two
packages would produce exactly the large, heterogeneous round where defects
hide — which this engagement has now seen twice, in RFC 107's binding test and
in the `json_shape` depth test, both of which passed while testing nothing.

**Against the standing constraint that process must not tax productivity:**
more stages is not more process. Each stage below carries *less* than a 2-stage
split would, because each is verified one way. The dispatches will be sized to
their stage, as `0.82.0`'s was.

## B — what is already built, so no stage re-does it

`crates/sui-id/src/http/dynamic_registration_validation.rs` (564 lines) holds
stage 1 and 1b's work: envelope bounds (`MAX_ENVELOPE_BYTES` 64 KiB,
`MAX_ENVELOPE_DEPTH` 16, `MAX_ENVELOPE_MEMBERS` 128), `parse_envelope`,
registration bearer-token extraction, the closed redirect profile
(`RedirectProfile`, `derive_closed_profile`, shape and percent-escape checks,
explicit-default-port handling) and `reject_http_post_logout_uris`.

Two sections of the matrix are therefore **done**: *Envelope and member
policy* and *Redirect corpus*. Five remain.

## C — the stages

### Stage 2 — the field rules

`metadata-validation.md` §*Supported fields* and §*Name and scope corpus*.
All of it pure validation in the existing module, all unit-testable without
HTTP.

- `client_name` — trimmed, 1–128 scalars, ≤512 bytes; reject blank,
  control/C0/C1, bidi override and isolate, oversize. The corpus names
  combining characters, emoji, HTML, quotes and log delimiters.
- `token_endpoint_auth_method` — `none`/`basic`/`post`, **default `basic`**.
- `grant_types` — `code`, or `code`+`refresh`; **default `code`**. Reject
  empty, duplicate, refresh-without-code, any other grant.
- `response_types` — `code` only; **default `code`**. **There is no
  implementation today**: `response_types` appears zero times in
  `handlers/dynamic_register.rs`. Measured, not assumed.
- `scope` — 1–32 unique catalog tokens including `openid`; RFC 6749
  scope-token characters only; 64/65-byte token and 1,024/1,025-byte total
  bounds; reject unknown, duplicate, malformed.
- application URI fields — absent, or one canonical HTTPS URI; reject HTTP,
  userinfo, fragment, non-canonical, oversize.
- **The coupling:** `refresh_token` requires `offline_access`, and
  `offline_access` requires `refresh_token`. Both directions.

**The corpus is the specification.** Where this summary and
`metadata-validation.md` differ, the matrix wins — say so rather than guessing.

### Stage 3 — the registration response

§*Response/error assertions*. The register handler's output, not its input.

- Success echoes every registered supported value **after defaults and
  canonical ordering**, plus the issued identifiers and secret timestamps.
- Ignored unknown metadata is **absent** from the echo.
- Invalid-token states are byte-identical in status, `WWW-Authenticate`, body
  and cache headers — one shape, no oracle.
- Public errors carry only fixed ASCII templates plus field/index identifiers.
- **Every success and every error carries `Cache-Control: no-store` and
  `Pragma: no-cache`.**

  *Corrected 2026-10-21, before stage 3 was dispatched: this said "neither
  header appears in `handlers/dynamic_register.rs` today", which was true of
  the handler and misleading about the route.* `Cache-Control: no-store` **is
  already set**, at `router.rs:33-41`, covering every response from
  `/oauth2/register`. Only `Pragma: no-cache` is absent. Stage 3's dispatch
  carries the correction.

  *Also found while measuring stage 3: the response has neither
  `client_id_issued_at` nor `client_secret_expires_at`, the second of which
  RFC 7591 §3.2.1 makes **required** when a secret is issued. That belongs to
  this stage and its dispatch says so.*

### Stage 4 — the authentication/grant matrix

§*Authentication/grant matrix*. **Separated because it is the security-bearing
part of what remains**: it is client authentication at the token endpoint, a
different file from stages 2 and 3, and the one stage where a wrong accept is
an authentication bypass rather than a validation gap.

Eight rows, each tested for **both** authorization-code and refresh dispatch:
`none` accepts only with PKCE and no secret anywhere; `basic` accepts exactly
one Basic pair and rejects a body secret; `post` accepts body credentials and
rejects a present Basic header; legacy accepts either but not both; **Basic and
body both present is always a reject**. Refresh is rejected unless grant policy
*and* scope policy both authorize it.

### Stage 5 — browser presentation

§*Browser presentation*. Separated because its verification is a browser
observation, not an assertion on a value.

- A dynamic `logo_uri` is stored and echoed but **never rendered or fetched**
  by consent or any other browser UI.
- Dynamic application links require a user action and carry
  `rel="noopener noreferrer"` plus a no-referrer policy.
- **A browser-network test must observe no request to a registrant-controlled
  logo origin** while dynamic-client consent is displayed. That is the stage's
  real deliverable; the first two are cheap without it.

### Stage 6 — the legacy-candidate report, read-only

Not in `metadata-validation.md`; it comes from RFC 095 `:350-360`, and it is
in this plan because **M3 cannot close without it and it is currently
unbuilt** — `legacy_unstamped_dynamic_candidate` appears only in the RFC, with
no code emitting it.

A **read-only upgrade report** listing positive `client.dynamic_register`
audit targets whose client row still says `admin`, labelling them
`legacy_unstamped_dynamic_candidate`, and recording where audit history is
absent or unverifiable. **It must not mutate them**, and it must not infer
provenance from shape, timestamps, names or missing application URIs — the RFC
forbids each by name.

It is sequenced last because it produces the list `@nabbisen` then disposes of,
and that disposition is a closure prerequisite rather than release content.

## D — what `0.83.0` contains, and what closes M3

**`0.83.0` = stages 2 through 6.** That completes M3's implementation.

**It does not close M3.** Two prerequisites remain afterwards and both are
`@nabbisen`'s:

1. **A durable disposition of every candidate stage 6 reports** —
   disablement, explicit legacy acceptance, or a separately audited metadata
   correction. *"No 'zero candidates' claim is allowed when the historical
   best-effort audit cannot prove completeness"*, so an empty report is not
   by itself an answer.
2. **Acceptance of the evidence**, on the architect's assessment of it.

Nothing else in M3's closure field is outstanding: the validate-first,
single-transaction behaviour and the consume-no-use / single-winner clauses
were met by C15 in cycle C.

## E — order, and what is dispatched now

Stages 2 → 3 → 4 → 5 → 6, one open dispatch at a time. Stage 2 is dispatched
with this plan: it is the largest, it blocks nothing, and stages 3 and 4
assert on values it defines.

**Stage 4 gets an independent design review before it is dispatched.** It is
the one security-bearing stage here, and the same rule that applied to 096's
security path applies to it.

## F — not in M3, and not proposed

M2b remains the programme after this, in tranches from `0.84.0`. The
embeddable and store-boundary decisions are untouched and still open; the
freeze holds, and nothing in this plan needs an exception to it.
