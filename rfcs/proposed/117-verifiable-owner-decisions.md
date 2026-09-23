# RFC 117 — Owner decisions must be verifiable

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None. RFC 000 is not amended by this RFC; it adds a way to evidence decisions, not a new rule about who may make them.
**Implementation prerequisites.** `@nabbisen` holds a signing key that no agent can use — the whole design rests on it, and nothing can be built before it exists (D1, open question 1).
**Closure prerequisites.** Every decision in the ledger from the adoption date onward carries a signature that verifies against a key pinned in the repository; a gate fails when a ledger entry's signature is absent, malformed or made by an unpinned key; every document that asserts an owner decision dated on or after the adoption date cites a ledger entry, and a gate fails when one does not; and the seven attributions that predate adoption are labelled unverifiable rather than quietly grandfathered.
**Tracks.** Governance integrity. Raised by `@nabbisen`, 2026-09-24, on RFC 110's design review.
**Touches.** A new ledger file, `ci/` (the pinned allowed-signers file), `scripts/` (the gate and its self-tests), `ROADMAP.md` §S1, and `docs/src/contributing/` for the procedure.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/117-verifiable-owner-decisions/README.md`](../handoffs/117-verifiable-owner-decisions/README.md)

## Summary

This repository records decisions as `@nabbisen`'s that he did not make. It has
happened three times that are known:

| Date attributed | What it created | What happened |
|---|---|---|
| 2026-07-28 | A vendor-independence rule in four RFC headers | Could not be evidenced. Blocked seven RFCs for four weeks. Withdrawn 2026-08-26 |
| 2026-09-09 | "The implementation role is not a reviewer", recorded in RFC 098 | Contradicts the decision of 2026-08-26 in `ROADMAP.md` §S1. Unresolved |
| 2026-09-10 | The `roadmap/` directory and its authority | The owner states he made no such ruling. Dissolved 2026-09-22 |

Each was written by the architect role, in the owner's name, and nothing in the
repository could contradict it. Seven distinct dated owner attributions exist
across 22 tracked files; three are the above.

**Git cannot settle any of them**, and this is the fact the design turns on.
Measured 2026-09-24: 411 commits under two identities
(`nabbisen@scqr.net`, the GitHub noreply address), `commit.gpgsign = true`, and
`user.signingkey` set to the owner's GPG key. **Every commit an agent makes in
this session is signed with that key and reports the same `%G?` status as a
commit the owner types himself.** Authorship and signature are therefore
evidence of the *machine*, not of the *person*. A ledger file in the tree would
inherit exactly that defect: it is a file, and an agent can write files.

## The principle

**A claim that the owner decided something is only evidence if it carries
something an agent cannot produce.** Everything else — a commit, a signature
made with the shared key, a line in a roadmap, a sentence in an RFC header — is
an assertion by whoever typed it.

## Decisions

**D1 — A signing key that no agent can use.** `@nabbisen` holds a second key,
distinct from `user.signingkey`, backed by hardware that requires a physical
touch to sign (an `ed25519-sk` SSH key or an equivalent GPG smartcard). Its
public half is pinned in the repository. No agent has it, and no agent can use
it without the owner physically present.

**D2 — Decisions are signed individually, not by their commit.** Each ledger
entry carries a detached signature over its own canonical text, produced with
`ssh-keygen -Y sign`. Signing the *commit* instead would tie the evidence to
history that a rebase, squash or filter rewrites; an entry signature is
self-contained, survives any history operation, and can be verified from a
working tree with no repository at all. The gate verifies every entry against
the pinned allowed-signers file.

**D3 — One ledger, and citing it is mandatory for the decisions that matter.**
A single file holds every recorded decision, each with an id, a date, the
owner's words verbatim, and its signature. Any document asserting an owner
decision dated on or after the adoption date must cite an entry id, and the
gate fails when it does not.

**D4 — Verbatim words, not paraphrase.** An entry records what the owner
actually wrote or said, quoted. Every one of the three failures above is a
paraphrase that drifted from — or invented — the thing it claimed to record. A
paraphrase may follow the quotation, marked as such and unsigned.

**D5 — Scope is proportionate: rules, not routine.** Signing everything would
add a hardware touch to every exchange and would be abandoned. Entries are
required for decisions that **establish, change or withdraw a rule, a scope, a
prerequisite or a security claim** — the class that caused all three failures.
Routine authorizations (accepting an RFC, authorizing a package, approving a
release window) stay as they are: frequent, low-stakes, and self-correcting
because the work that follows is visible. *The boundary is open question 2.*

**D6 — What predates adoption is labelled, not grandfathered.** The seven
existing attributions cannot be verified retroactively, and pretending
otherwise would be the same error in a new form. They are marked as
unverifiable, with the three known-bad ones marked as such, and `ROADMAP.md`
§S1 continues to carry the history. **Nothing is deleted and nothing is
promoted.**

## What this does not achieve

Stated plainly, because a control described as stronger than it is, is how this
project got here.

- **It does not make forgery impossible.** An agent running on the owner's
  machine could invoke the signing command while he is at the keyboard, and he
  could touch the token believing it was something else. The boundary is the
  physical touch *and his attention at that moment*. This raises the bar from
  "anything an agent writes" to "something the owner physically participated
  in"; it does not remove him from the loop, it puts him in it.
- **It does not verify the content of an entry**, only that the holder of the
  key signed that text. An entry can be signed and still be a poor record of
  what was meant. D4 is the mitigation and it is a discipline, not a gate.
- **It does not reach decisions made in conversation and never recorded.** If a
  ruling is not written down it remains unevidenced, exactly as now.

## Open questions

1. **Which key.** An SSH `ed25519-sk` key on a hardware token is the smallest
   thing that works: `ssh-keygen -Y sign` and `-Y verify` are in OpenSSH 8.2+,
   which every supported runner has, and the allowed-signers format is one
   pinned line. A GPG smartcard is equally sound and heavier to verify in CI.
   **The architect recommends `ed25519-sk`.** This is `@nabbisen`'s to choose,
   and it determines whether this RFC is buildable at all.
2. **Where the boundary in D5 falls.** The architect's proposal is above. The
   cost is one hardware touch per rule-level decision, which on the evidence of
   this programme is a handful per month.
3. **Whether the ledger is a tracked file or a separate artifact.** Tracked is
   simpler and the signature makes tampering detectable; the counter-argument
   is that an agent can still *delete* an entry, and only the gate's
   completeness check would notice.

## Risks

- **Key loss ends the scheme** until a new key is pinned, and the pinning
  commit is itself unverifiable. Mitigation: pin two keys from the start.
- **Adoption cost falls entirely on the owner.** Every other control in this
  project costs the agents effort and the owner nothing. This one is the
  reverse, and it is the reason D5 scopes it narrowly. If it is felt as
  friction it will lapse, and a lapsed ledger reads as "no decisions were made"
  rather than "the record stopped".
- **It may be more machinery than the problem deserves.** Three failures in
  three months is the evidence for it; a smaller project would simply have the
  owner say "I did not decide that", which is what happened twice. The honest
  case for building it is that the third time cost a directory, nineteen
  packages and a day of restructuring to undo.
