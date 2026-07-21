# RFC 094 federation-command amendment independent review and owner approval

**Review date.** 2026-07-21 (Asia/Tokyo)
**Amendment.** RFC 094 C17/C18/C23/F01–F06 for RFC 096 federation
**Verdict.** Accept with notes
**Independent reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex)
**Approved on.** 2026-07-21 (Asia/Tokyo)
**Approved by.** `@nabbisen`
**RFC author / architect.** `codex-project-architect` (OpenAI Codex)
**Implementation owner.** `codex-developer` (OpenAI Codex), subject to the RFC entry gates

## Artifact identity and provenance

The independent third-amendment review examined repository baseline
`0b1639b6c7926a811dd04b355e0f2c4aed1f1da8` plus a then-unstaged synchronized
package. That reviewer did not publish independently observed file digests or
a repository snapshot for the historical working-copy bytes. This durable
record therefore does not claim that author-calculated historical digests are
independent evidence.

The return-to-Proposed table below was author-observed, independently matched
12/12 in focused review, and is now durably retrievable as commit
`43085e38219e5eb1bfe11cc698b18f1fa5f5e4d7`. It is the exact input identity for
the later Accepted transition.

| Return-to-Proposed candidate path | SHA-256 |
|---|---|
| `rfcs/proposed/094-transactional-audit-registry.md` | `20d826e22791af4890a83ff17c5fd48d326b706ba175f4d6117655d92a722fed` |
| `rfcs/handoffs/094-transactional-audit/README.md` | `87bbc3366bd5c40ea5b60cd0c759b96c39372eb1cb191d6c24a68d62ee1310e8` |
| `rfcs/handoffs/094-transactional-audit/architecture.md` | `a7f963c0f32a92a13fecfd46fc6ae12c746fdf06bb112c5eac2558346f5ad1f1` |
| `rfcs/handoffs/094-transactional-audit/command-inventory.md` | `eb05b8333127a33014f315a52ec4b071cb6ba38f80844db8718570cc8a9a8dac` |
| `rfcs/handoffs/094-transactional-audit/migration-checklist.md` | `7dcf78ea7fa9a03dcf1847fe585e470c6f897841fc94f257740455e1ae996b99` |
| `rfcs/handoffs/094-transactional-audit/verification.md` | `03d38e9d2a49374ad2d93ff8a6dccd4d15426003e2e5d3d75344c473660b8599` |
| `rfcs/proposed/096-upstream-oidc-federation-validation.md` | `acce0992b103cab7a2d09826cc27a19306c6456ab36ad66eefd8e52b3dab150d` |
| `rfcs/handoffs/096-upstream-oidc-federation/README.md` | `90700f5e1018ed6c46be695e06f1cfcfbe538e89e6ac782adb2e7afe8ee7efef` |
| `rfcs/handoffs/096-upstream-oidc-federation/architecture.md` | `abc9dcf4a79a5613290dea46e2fe9bb83f38192a89c7df90d933a14985f7ad28` |
| `rfcs/handoffs/096-upstream-oidc-federation/validation-matrix.md` | `a05dc97f36559d66ce3e7b1eb0918c4dfda34e2644ae96d85bd826028d38b8c0` |
| `rfcs/handoffs/096-upstream-oidc-federation/verification.md` | `d31e0879238447c4b8dcf22adcb976af76e3e0b27c72cb367bc5c95dca0934c4` |
| `rfcs/README.md` | `d93c8afde6e6050c3695cb5494d0b48c863ab5279ae952c381c5edf62f7b9060` |

The Accepted-transition table binds the exact output candidate. Its digests are
author-observed and reproducible from the named workspace files; the focused
reviewer must independently recalculate them.

| Accepted-transition candidate path | SHA-256 |
|---|---|
| `rfcs/accepted/094-transactional-audit-registry.md` | `2c3e36e89f2d45f6aca8fec804f61abc793957dc6bc03fe7d020672469be1aa7` |
| `rfcs/handoffs/094-transactional-audit/README.md` | `b0a9ad6706c819883fecf9a497aabc29c40c6e8359fbb3bfd9bc014eb172c8cd` |
| `rfcs/handoffs/094-transactional-audit/architecture.md` | `5cd5e0a7baaf9356fd1c37b58af6fc004cb5c2b125c0863633581020a3f98fe2` |
| `rfcs/handoffs/094-transactional-audit/command-inventory.md` | `b4d505f0d591ac7e6ec45ec7b5cde4b173fdc4a58b7275a39cb3c84ddb50e718` |
| `rfcs/handoffs/094-transactional-audit/migration-checklist.md` | `0df6c2f71a6eeaa307795c0ed60d1a8b9b9f7e0889b2436e4a19732c6deb9b16` |
| `rfcs/handoffs/094-transactional-audit/verification.md` | `4855b331d49e86aef955bc13d4729ec9addab06c4cb5191a3f16bccb53b001b2` |
| `rfcs/proposed/096-upstream-oidc-federation-validation.md` | `bdcf3fbe1e395c0d904339c39a790735f94a41eea26739e38b729fc19274abda` |
| `rfcs/handoffs/096-upstream-oidc-federation/README.md` | `5c88f98403837934a672b94f653184237055b59dada9f1faebe55e8c535412f2` |
| `rfcs/handoffs/096-upstream-oidc-federation/architecture.md` | `d68d3c8d4e7fb15efc2ab81c33833307e4f95c9f9563858212f1a3507bbfa9a8` |
| `rfcs/handoffs/096-upstream-oidc-federation/validation-matrix.md` | `a05dc97f36559d66ce3e7b1eb0918c4dfda34e2644ae96d85bd826028d38b8c0` |
| `rfcs/handoffs/096-upstream-oidc-federation/verification.md` | `d31e0879238447c4b8dcf22adcb976af76e3e0b27c72cb367bc5c95dca0934c4` |
| `rfcs/README.md` | `3551fea75f9ff8c69c39d8fe908693b1bfe45010c8b4e36ca628731317f2f1be` |

Relative to the durable input commit, the output changes only RFC 094 lifecycle
path/status/metadata/history, index and handoff links/status, accepted-amendment
labels, RFC 096 prerequisite wording, this output identity, and the two
editorial corrections accepted by the return review. It does not accept RFC
096, claim implementation evidence, or authorize implementation before gates.

## Review chain and decision

The design required four review rounds. The initial review required closure of
preflight durable-write ownership, federated-MFA attempt/ceremony authority,
and hostile HTTP allocation. The first and second amendments closed the MFA,
HTTP, NAT64, event, and cache issues but exposed a concurrent preflight replay
after disable. The third amendment introduced a checked durable activation
generation across C17/C18/C23 and propagated it through attempts, MFA,
ceremonies, cache, single-flight, and conditional reuse.

The final independent verdict was **Accept with notes** and explicitly found:

- no blocking finding remained;
- RFC 094 C17/C18/C23/F01–F06 was ready for owner approval;
- RFC 096 was ready for its separate acceptance decision after that approval;
  and
- implementation remained NO-GO pending lifecycle and implementation gates.

`@nabbisen` then explicitly approved this RFC 094 amendment. That decision is
preserved as decision history. RFC 000 then required and received the durable
return-to-Proposed commit identified above; focused review accepted it with all
12 hashes matching. `@nabbisen` subsequently explicitly accepted the complete
materially amended RFC 094. None of those decisions accepts RFC 096.

## Accepted implementation notes

The synchronized governing and handoff documents freeze the final review's
non-blocking notes:

1. C16 provider IDs are globally fresh and never reused after C18; a future
   deterministic/operator ID scheme needs tombstone/incarnation authority.
2. Startup reconciliation preserves an unchanged sealed secret envelope and
   does not treat randomized resealing as a policy change.
3. Operator preflight/live-canary evidence records the intentionally narrow
   HTTP/1.1 interoperability profile without a generic-client fallback.
4. Already-disabled C17 disable is a non-committing no-op, not an implicit
   cancellation command; cancellation or an explicit generation advance is
   required to revoke an in-progress disabled-state enable operation.

These notes add defense-in-depth and operational precision without weakening
the reviewed activation, policy-digest, MFA, transport, cache, event, or
lifecycle invariants.

## Lifecycle and implementation boundary

RFC 094 is Accepted with complete current metadata and durable Proposed input
provenance. RFC 096 remains Proposed until `@nabbisen` separately accepts it and
its own transition passes focused review. Implementation remains prohibited
until each RFC 093/094/096 implementation prerequisite and ownership gate has
repository-visible passing evidence.
