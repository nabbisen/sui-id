# RFC 096 independent design review and acceptance record

**Review date.** 2026-07-21 (Asia/Tokyo)
**RFC.** RFC 096 — Upstream OIDC Federation Validation
**Verdict.** Accept with notes
**Independent reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex)
**Accepted on.** 2026-07-21 (Asia/Tokyo)
**Approved by.** `@nabbisen`
**RFC author / architect.** `codex-project-architect` (OpenAI Codex)
**Implementation owner.** `codex-developer` (OpenAI Codex), subject to every entry gate

## Owner decision scope

After the complete materially amended RFC 094 became durably Accepted in commit
`336b064686017ba5adb65ec26c50e679881f8adf`, the owner was asked for the
separate RFC 096 acceptance decision and answered `OK. Start it.` This record
treats that response as authorization to prepare the RFC 096 Accepted
transition for focused review. It does not authorize implementation start,
which remains gated below.

## Durable input identity

The transition starts from clean commit
`336b064686017ba5adb65ec26c50e679881f8adf` (`Accept complete amended RFC 094`).
Its exact RFC 096 input files are:

| Proposed input path | SHA-256 |
|---|---|
| `rfcs/proposed/096-upstream-oidc-federation-validation.md` | `bdcf3fbe1e395c0d904339c39a790735f94a41eea26739e38b729fc19274abda` |
| `rfcs/handoffs/096-upstream-oidc-federation/README.md` | `5c88f98403837934a672b94f653184237055b59dada9f1faebe55e8c535412f2` |
| `rfcs/handoffs/096-upstream-oidc-federation/architecture.md` | `d68d3c8d4e7fb15efc2ab81c33833307e4f95c9f9563858212f1a3507bbfa9a8` |
| `rfcs/handoffs/096-upstream-oidc-federation/validation-matrix.md` | `a05dc97f36559d66ce3e7b1eb0918c4dfda34e2644ae96d85bd826028d38b8c0` |
| `rfcs/handoffs/096-upstream-oidc-federation/verification.md` | `d31e0879238447c4b8dcf22adcb976af76e3e0b27c72cb367bc5c95dca0934c4` |
| `rfcs/README.md` | `3551fea75f9ff8c69c39d8fe908693b1bfe45010c8b4e36ca628731317f2f1be` |

## Accepted-transition output identity

The output table binds the exact current candidate and must be independently
recalculated during focused transition review. This record itself is reviewed
directly and omitted from its own table to avoid recursion.

| Accepted output path | SHA-256 |
|---|---|
| `rfcs/accepted/096-upstream-oidc-federation-validation.md` | `5ee73fd1daa26e575194deaa3ebdcf4c9a1eeb96c5202c6acf8176d838174158` |
| `rfcs/handoffs/096-upstream-oidc-federation/README.md` | `4e0293eec43b33eacf92c717270f20517d855483a30b62b3639cae22ad8d76d3` |
| `rfcs/handoffs/096-upstream-oidc-federation/architecture.md` | `d68d3c8d4e7fb15efc2ab81c33833307e4f95c9f9563858212f1a3507bbfa9a8` |
| `rfcs/handoffs/096-upstream-oidc-federation/validation-matrix.md` | `a05dc97f36559d66ce3e7b1eb0918c4dfda34e2644ae96d85bd826028d38b8c0` |
| `rfcs/handoffs/096-upstream-oidc-federation/verification.md` | `d31e0879238447c4b8dcf22adcb976af76e3e0b27c72cb367bc5c95dca0934c4` |
| `rfcs/README.md` | `06b337a2a21254b9f8cf6fbb674d12bb95a6439759d87be62820964d9baf3645` |

The transition changes only the RFC path/header acceptance metadata, handoff
governing path/inherited lifecycle label, index row, and this durable record.
The RFC body from `## Summary` through end-of-file and the architecture,
validation matrix, and verification companions remain byte-identical to the
durable Proposed input. The RFC body identity from `## Summary` through EOF is
`f00a1ee3289d3bdc69d73b89fd8a10967b425b7e0b25cd013fdd5552c065cd9d`
for both input and output.

## Review chain and design decision

RFC 096 required four substantive design rounds and two RFC 094 lifecycle
repairs:

1. the initial review required a single auditable federation command boundary,
   safe activation/preflight authority, a complete local-MFA state machine,
   and hard hostile-HTTP allocation limits;
2. amendments closed standalone preflight writes, five-attempt F02/F03/F06
   ownership, the HTTP/1.1 parser/framing boundary, NAT64 policy, closed event
   reasons, and narrow-cache semantics;
3. the final security blocker—pre-disable sibling preflight replay—was closed
   with checked durable `activation_generation` across C17/C18/C23 and every
   attempt/MFA/ceremony/cache/304 authority; and
4. the final independent design verdict was **Accept with notes**, with no
   blocking finding and explicit GO for RFC 096 acceptance after RFC 094's
   material amendment became lifecycle-valid.

RFC 094 subsequently followed RFC 000's required return-to-Proposed and
complete reacceptance sequence in commits
`43085e38219e5eb1bfe11cc698b18f1fa5f5e4d7` and
`336b064686017ba5adb65ec26c50e679881f8adf`. RFC 096's design prerequisite is
therefore durable.

## Accepted implementation notes

The final non-blocking notes are frozen in the governing/handoff package:

1. provider IDs are globally fresh and never reused after delete;
2. unchanged client-secret plaintext preserves its existing sealed envelope;
3. the intentionally narrow HTTP/1.1 compatibility profile is recorded in
   operator preflight/live-canary evidence with no generic-client fallback; and
4. already-disabled C17 disable is a no-op, not implicit preflight
   cancellation; cancellation or explicit generation advance is required.

The policy digest, private MFA verifier, rustls allocation evidence, exact
Content-Length drop behavior, address policy, cache/304 rules, events, and
activation-generation tests remain mandatory.

## Lifecycle and implementation boundary

Acceptance makes RFC 096 design-approved but does not start implementation.
Pure stages 1–3 remain blocked until RFC 093 is Implemented with its clean-tree
matrix, a clean implementation baseline and non-overlapping file ownership are
recorded, and the hostile-provider harness is approved. Mutation/session stage
5 additionally waits for amended RFC 094 to be Implemented with all applicable
commands and fixtures passing. Any RFC 095 overlap requires explicit ownership
release and the roadmap's second implementer/independent reviewer capacity.

No build, test, migration, transport, browser, live-provider, packaging, or
runtime evidence is claimed by this design acceptance.
