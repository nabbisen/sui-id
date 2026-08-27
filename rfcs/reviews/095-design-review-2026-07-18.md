# RFC 095 independent design-review approval

**Review date.** 2026-07-18 (Asia/Tokyo)
**RFC.** RFC 095 — Dynamic Client Registration Transaction and Validation
**Verdict.** Accept with notes
**Independent reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex)
**Accountable project owner.** `@nabbisen`
**RFC author / architect.** `codex-project-architect` (OpenAI Codex)
**Implementation owner.** `codex-developer` (OpenAI Codex)

## Reviewed artifact identity

The independent review examined repository baseline
`20897e7fae8da805994be48d0e356295a6f3b6ef` plus the then-unstaged RFC 095
package. Unlike RFCs 093 and 094, the reviewed proposal package did not have an
intermediate repository commit. The exact reviewed bytes are therefore bound
directly by SHA-256:

| Reviewed proposal path | SHA-256 |
|---|---|
| `rfcs/accepted/095-dynamic-client-registration-transaction.md` | `feaf4098ed4eecb7b8f083660fc85431327a8372d57dcf8c86bd5866ed8d9e25` |
| `rfcs/handoffs/095-dynamic-client-registration/README.md` | `31aa6dce02b8754552509c93da5c57dc261f3d0f6b3ef307412526f4590171d0` |
| `rfcs/handoffs/095-dynamic-client-registration/architecture.md` | `cd8c8ebac2b7ad47dda743432b2501b1b1d38b21bdea70fe624de4338570174e` |
| `rfcs/handoffs/095-dynamic-client-registration/metadata-validation.md` | `8dd29e038b5425b41fbcfacc1bba304e08bcd9c32d9881ebb0f465a512d628da` |
| `rfcs/handoffs/095-dynamic-client-registration/verification.md` | `676428a4be134775c7ceb60229d201e1a09d9de8745e51b164d16509ab1f6d0c` |

The lifecycle transition changes only the RFC header/location and index. Its
design body from `## Summary` through end-of-file remains byte-identical to the
approved proposal and has SHA-256:

`da95231915e37c3d0ed79b66dd65032f4cf378387969cc36bb94e22a1be246cc`

The metadata-validation companion remains byte-identical. The handoff README
changes its governing path and inherited lifecycle label and names the frozen
implementation-note contract. Architecture and verification add only the
concrete constants, storage/consumption mechanics, and evidence required by
the final review's accepted N1/N2 notes. Those additions do not change the
approved logout, redirect, transaction, CORS, migration, or privacy invariants
and are subject to the Accepted-transition review.

## Review chain and decision

The independent review approved RFC 095 after three rounds:

1. the initial detailed-design review required changes for native-loopback
   client classification, authenticated exact logout, C15 possession binding,
   transaction-time expiry, blocking-worker cancellation, revocation-safe
   CORS, and dynamic-logo browser privacy;
2. the first amendment closed six blockers but required HTTPS-only
   profile-independent logout and explicit invalid-hint confirmation; and
3. the second amendment resolved that final blocker and returned
   **Accept with notes — GO for design-acceptance readiness**.

The accepted design:

- derives one persisted closed redirect profile and confines variable-port
  loopback matching to PKCE public-native authorization;
- makes every new dynamic logout URI independent HTTPS, exact, verified-hint
  bound, and free of bare-client or redirect-list fallback;
- supports bounded GET and POST logout parsing while requiring a separate
  CSRF-protected local confirmation POST for invalid, missing, expired-outside-
  policy, or mismatched hints;
- binds token ID and a private zeroizing possession digest in one sealed C15
  authority and rechecks expiry against a trusted time captured after
  `BEGIN IMMEDIATE`;
- preserves post-dispatch cancellation ambiguity instead of claiming false
  rollback or retry safety;
- makes CORS revocation linearizable through a canonical, checked, guarded
  durable generation;
- applies total, non-guessing legacy-profile migration; and
- prevents dynamically supplied logos from becoming automatic server or
  browser requests.

No blocking design finding remains.

## Accepted implementation notes

Before implementation begins, the handoff freezes the final review's
non-blocking notes:

- logout GET/POST share named 16,384-byte whole-request, eight-member, and
  8,192-byte scalar limits;
- the local confirmation capability uses 32 CSPRNG bytes, a 300-second lifetime
  with `now < expires_at`, and at most one outstanding value per session;
- only a SHA-256 digest is stored, while the raw unpadded-base64url value is
  limited to the hidden no-store form and POST body and is redacted/zeroized
  elsewhere;
- creation replaces the prior session/purpose value atomically; guarded
  confirmation consumption and local session logout share one immediate
  sealed protocol transaction, so simultaneous duplicates have one winner;
- cleanup occurs at startup, periodically, opportunistically, and through
  session-delete cascade; and
- confirmation pages preserve no-store, `frame-ancestors 'none'`, and
  `X-Frame-Options: DENY`.

These constants and mechanisms implement the already-approved confirmation
boundary. Changing them materially requires return to architecture/security
review; they do not import administrator authority from the existing pending
settings-change path.

## Independence attestation

The named independent reviewer performed the design-review execution and did
not author RFC 095 or act as its implementation agent. The role separation is
process separation within OpenAI Codex; organizational or vendor independence
is not claimed.

## Owner authorization and implementation boundary

On 2026-07-18, after the final independent review returned design GO,
`@nabbisen` explicitly authorized RFC 095's atomic Accepted transition and
confirmed the named implementation owner.

This authorization does not authorize implementation. RFC 095 implementation
remains prohibited until RFC 094 is Implemented with its C15 baseline evidence,
RFC 093's applicable clean-baseline matrix passes on the selected implementation
commit, the migration number and non-overlapping ownership are recorded, and
every handoff entry gate is satisfied.

## Observed evidence boundary

The independent reviews examined the RFC/handoff design, relevant current
registration, token, logout, session/CSRF, CORS, consent-logo, migration, clock,
and blocking-worker paths, plus the named primary specifications. The final
review observed no blocking finding and no regression in prior closures.

This is a design decision record. It does not claim that RFC 094 is Implemented
or that any RFC 095 build, test, clippy, migration, browser, logout, CSRF,
fault-injection, concurrency, fuzz, LDAP, mdBook, package, integration, or
runtime gate passed.
