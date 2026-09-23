# RFC 117 — independent design review request

**RFC.** [RFC 117 — Owner decisions must be verifiable](../../proposed/117-verifiable-owner-decisions.md). Proposed.
**Reviewer.** Mid-capability model, implementation role. It authored neither the
RFC nor its handoff.
**Route.** Owner decision of 2026-08-26 (`ROADMAP.md` §S1): a design is reviewed
by the role that must build against it.
**Why now.** `@nabbisen` accepted this RFC on 2026-09-24. RFC 000 requires a
named independent design reviewer for a security-sensitive RFC, and G11 refuses
an Accepted RFC whose `Independent design review` field has no durable
reference.
**Baseline.** `8741fa6` or later.
**Scope.** Read-only. Change no code and no RFC text. Report findings.

## Read this first: the objection the RFC does not answer

The architect raises it against his own design, because it may be fatal and it
should be confronted before anything is built.

**The verifier lives in the same tree as the thing it verifies.** The argument
of this RFC is that an agent can write files, so a ledger file is not evidence.
But `scripts/`, `ci/` and `.github/workflows/` are also files. An agent that
writes a ledger entry the gate rejects can, in the same commit, weaken the gate
— or the allowed-signers file, or the workflow that runs it — and CI goes
green. Nothing in the repository can stop that, because everything in the
repository is writable by whoever writes the repository.

So the scheme may reduce to: **tampering becomes visible in a diff, if someone
reads the diff.** That is not nothing — every one of the three failures went
unnoticed for weeks precisely because nothing made them visible — but it is a
much weaker claim than "the owner's decisions become verifiable", and RFC 117
currently makes the stronger one.

1. **Is that objection correct?** State it precisely, in your own terms, and
   say how far it goes. If it is correct, does it sink D1–D3, or does it change
   what they claim?
2. **Is there a construction where the verifier is outside the agents' reach?**
   Candidates to assess, and any others you find: GitHub branch protection with
   a required status check configured server-side; `CODEOWNERS` requiring the
   owner's review on `ci/`, `scripts/` and `.github/`; signed tags verified at
   release rather than per commit; verification run somewhere that is not this
   repository. For each: does it actually escape the problem, or does it move
   it? Say what an agent with commit access to this repository still could do
   under each.
3. **If nothing fully escapes it, what is the honest claim?** Draft the sentence
   RFC 117 should make instead. The architect would rather ship a weaker true
   claim than a stronger false one — that failure mode is what this RFC exists
   to correct.

## 1. Is the cryptography sound as specified?

4. **`ssh-keygen -Y sign` / `-Y verify`.** Confirm availability and version on
   `ubuntu-24.04` runners and on the pinned toolchain, the exact invocation and
   the allowed-signers file format, and whether a namespace is required. Say
   what the verify command's exit codes are for: good signature, wrong key,
   malformed signature, missing signer.
5. **The canonical-text problem, which the handoff calls the one that matters
   most.** The failure mode is an entry whose signature is valid but covers
   different bytes than the entry *displays*. Propose a format where the signed
   region is delimited literally rather than reconstructed by a parser, and say
   how a reader with no tooling can see exactly what was signed. Is a detached
   signature over a delimited block sufficient, or does the entry need to embed
   its own digest?
6. **Per-entry signatures versus commit signatures.** The RFC chose per-entry so
   the evidence survives a rebase. **Challenge it.** What does a commit
   signature give that an entry signature does not — in particular, does it bind
   the decision to a *time*, which an entry signature does not? Is a countersigned
   entry (both) worth the cost?
7. **Hardware-backed keys.** `ed25519-sk` requires a touch, but confirm: does a
   resident or non-resident `-sk` key require the touch on **every** signature,
   or can `no-touch-required` be set at generation — and can that be detected
   from the public key alone, so the gate can refuse a key that does not require
   a touch? If it cannot be detected, the "physical touch" property is
   unverifiable and the RFC should say so.

## 2. Is the scheme buildable and proportionate?

8. **Stage 2's citation rule.** Any document asserting an owner decision dated
   on or after adoption must cite an entry. The hard part is the exceptions —
   this RFC quotes historical attributions three times, and `ROADMAP.md` §S1 is
   the history of the bad ones. **RFC 110's design review found that a lexical
   carve-out anyone can write is a hole.** Propose an exception mechanism that
   is not one, or say it cannot be done and what that costs.
9. **False positives and the phrasing problem.** The seven attributions were
   found by a loose regex, and the RFC 110 review noted the count depends on
   the regex. How would you detect an *assertion of an owner decision* without
   either banning the required header fields (`Approved by`, `Accepted on`) or
   matching nothing?
10. **Scope (D5): rules, not routine.** Is the boundary — "establishes, changes
    or withdraws a rule, a scope, a prerequisite or a security claim" —
    decidable by a person in the moment, or will it be argued each time? If it
    will be argued, propose a sharper test, or say the scope should be wider or
    narrower.
11. **The adoption cutoff (D6).** The seven pre-adoption attributions cannot be
    signed retroactively. Is "label them unverifiable" the right treatment, and
    what should happen where one of them is **load-bearing for a rule still in
    force**? Name any you find in that position.

## 3. The threat model, stated properly

The RFC does not state one, which is a gap.

12. **Who is the adversary?** Three candidates, and the answer changes the
    design: (a) an agent that is helpful but wrong — it believes the owner said
    something and records it in good faith, which is what happened all three
    times; (b) an agent that is adversarial; (c) a third party with repository
    access. For each, say which of D1–D6 helps and which does nothing.
13. **If (a) is the real adversary** — and the evidence says it is — is a
    hardware key the proportionate control, or would a cheaper mechanism catch
    the same failures? Assess at least: requiring the owner's verbatim words
    with no paraphrase (D4 alone, no cryptography); a gate that simply *lists*
    every owner attribution in a CI summary so it cannot pass unseen; and a
    convention that an agent may never write the phrase at all, only quote a
    file the owner maintains. Say which of the three failures each would have
    caught. **Be concrete: name the incident and the mechanism.**
14. **What does the owner actually do per decision?** Write out the steps, end
    to end, for a real ruling — from him typing a sentence in chat to a signed
    entry landing. If it is more than about a minute, say so, because RFC 117's
    own Risks section says a scheme felt as friction will lapse.

## What to return

A review-request package under `.git-exclude/review-requests/`, containing:
- your answer to items 1–3 first, as its own section — it decides whether the
  rest matters;
- findings ranked blocker / high / medium / low;
- the measured answers to items 4 and 7, with the commands you ran;
- your answers to items 5, 6, 8–14, each citing what you read;
- a recommendation: build as specified, build with the changes you name, or do
  not build, with reasons.

**A recommendation not to build is a legitimate outcome of this review** and
will not be treated as a failure to deliver. The architect has an interest in
this RFC being right, not in it being built.

Do not implement anything. This RFC is Proposed; implementation is not
authorized until it is Accepted, and it additionally cannot start until
`@nabbisen` holds a key no agent can use.
