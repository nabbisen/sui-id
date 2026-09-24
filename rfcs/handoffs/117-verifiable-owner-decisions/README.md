# RFC 117 implementation handoff — verifiable owner decisions

**Governing RFC.** [RFC 117](../../proposed/117-verifiable-owner-decisions.md), **Proposed**.
Nothing here is authorized until it is Accepted.
**Implementer.** Mid-capability model.
**Baseline.** The commit that adds this file, or later.
**Prerequisites, split 2026-09-24 on the design review's recommendation.**
**Stage 0 needs no key** and should be built first: it is the control that
actually works against the real adversary, and it would have caught all three
past failures. **Stages 1–3 need the key**, and do not begin them by stubbing
it — a gate whose only fixture is a stub is a gate that has never run.

## The measurements this RFC rests on

Re-run before starting; a disagreement is a blocker.

| Claim | Command |
|---|---|
| 411 commits, two identities | `git rev-list --count HEAD`; `git log --format=%ae \| sort \| uniq -c` |
| commits are signed with the shared key | `git config --get commit.gpgsign`; `git config --get user.signingkey` |
| an agent's commit is indistinguishable | `git log --format="%h %G? %ae" -10` — every recent commit, agent-made, reports `U` |
| seven dated owner attributions across 22 files | `grep -rEoi "owner (ruling\|authorization\|decision\|approval)[,:]? *,? *2026-[0-9]{2}-[0-9]{2}" --include="*.md" .` |

## Order

| Stage | Content | Prerequisite |
|---|---|---|
| **0** | **The census and the closed baseline (D0)** — no key needed | RFC Accepted |
| 1 | The ledger format, the pinned allowed-signers file, and the verify gate | stage 0; **the key exists** |
| 2 | The citation rule wired to the baseline | stage 1 |
| 3 | Labelling the pre-adoption attributions (D6) | stage 1; **the owner's word on the two load-bearing ones** |

## Stage 0 — the census and the baseline (no key)

Match **generously**, not narrowly: the owner or `@nabbisen` within a line of a
decision verb stem (`rul`, `decid`, `decision`, `authori[sz]`, `approv`,
`accept`, `direct`, `instruct`), **regardless of date**. The narrow pattern in
the measurements below **misses the 2026-09-09 attribution**, which is one of
the three known-bad ones — verified. A gate keyed to one pattern misses the
incident you already know about.

- **Print every hit in the CI summary.** The census is half the value: two of
  the three failures were single commits that nobody looked at.
- **Record the closed baseline at the adoption commit** — path plus normalised
  text hash for every attribution that then exists. The cutoff is the tree, not
  a date in a sentence.
- **Fail only** on a hit that is neither baselined nor citing a ledger entry.
  Clearing a false positive is an edit to the baseline file, in a visible diff —
  not a marker the author types into their own paragraph.

**Evidence.** The three historical commits, replayed: `1e59e3d` (2026-07-28),
RFC 098's clause (2026-09-09) and `ee48257` (2026-09-10) each introduce an
attribution the gate would reject. The real tree passes without editing any
document. A new attribution with no date fails. A baselined one passes.

**This stage can be built today and is the one to start with.**

## Stage 0b — print what the baseline edit changed — dispatched 2026-09-24

Stage 0 landed with a gap its own implementer named, and I reproduced: a commit
that adds an attribution **and** runs `--update-baseline` passes G16 with "0
new". The only trace is the diff of `ci/owner-attribution-baseline.txt`, and two
of the three past failures were single commits nobody read.

It compounds with the other finding: every RFC acceptance trips the gate on its
own `Approved by` header until someone baselines it, so **baseline edits will be
routine — and a routine laundering path is not a control.**

**Required.** The G16 job fetches enough history to diff against the base, and
prints `git diff <base> -- ci/owner-attribution-baseline.txt` into the step
summary, under its own heading, **above** the census. What clears a hit is then
printed beside the hit it clears.

**Not required, and deliberately not chosen:** failing when the baseline changes
without a matching pin elsewhere. The pin is agent-writable too, so it buys
ceremony rather than evidence.

**Evidence.**
- A branch that adds an attribution and baselines it in one commit shows the
  added baseline line in the summary.
- A branch that touches neither shows an empty section, not a broken one.
- **Say what happens when there is no base** — a first push, a tag, a detached
  build — and make it degrade to a stated message rather than an obscure
  failure. That case is the one most likely to be met first and least likely to
  be tested.

This does not need the key, and it gets stronger once the credential split the
design review raised is decided.

## Stage 1 — the ledger and its gate

**The ledger.** One tracked file. Each entry carries: an id (`D-0001`, stable
and never reused), an ISO date, the decision **in the owner's own words,
quoted**, an optional paraphrase marked as such, and a detached signature.

**Canonical text.** Define exactly which bytes are signed — the id, the date
and the quoted words, in a stated order and encoding, excluding the signature
itself and any paraphrase. Write it down in the file's own header. **Every
later verification depends on this being unambiguous**; if two readers can
disagree about which bytes are covered, the signature proves nothing. Prefer a
format where the signed region is delimited literally rather than reconstructed
by a parser.

**The pinned key.** An `ssh-keygen` allowed-signers file under `ci/`, holding
the public half and a namespace. Pin **two** keys from the start (RFC 117
Risks): losing the only key ends the scheme, and the commit that pins a
replacement is itself unverifiable.

**The gate.** For every entry: the signature verifies, with the pinned
allowed-signers file and the declared namespace, over the canonical text. Any
entry that fails, is unsigned, or is signed by an unpinned key fails the gate.
Wire it as a lane through `scripts/ci-gate.sh` like every other — do not give it
its own entry point, or it becomes the second G12.

**Evidence.**
- A real entry, signed by the real key, verifying in CI.
- Mutations, each caught, each restored: one byte changed in the quoted words;
  the date changed; the signature truncated; the signature replaced with one
  made by a different key; an entry with no signature; the allowed-signers file
  pointed at a key that did not sign.
- **A negative test that matters more than the rest:** an entry whose signature
  is valid but covers different text than the entry displays. If the canonical
  text is reconstructed loosely, this passes and the whole scheme is decorative.

## Stage 2 — the citation rule

Any document asserting an owner decision dated **on or after the adoption
date** must cite a ledger entry id. The gate fails when one does not.

- Search the same class of phrases the measurement above uses.
- **The pre-adoption cutoff is the whole design of this stage.** Retrofitting
  citations onto the seven existing attributions is impossible — they have no
  entries and cannot get one — so the rule is dated, and the date is stated in
  the ledger's header.
- False positives to handle: a document *quoting* a historical attribution
  (this RFC does it three times), and `ROADMAP.md` §S1, which is the history of
  the failures and must keep naming the bad attributions. Decide whether these
  are handled by scope (a fenced block, a named file allowlist in `ci/`) or by
  a marker, and say why in the review package. **Do not choose a lexical
  carve-out that any author can write** — that is the mistake RFC 110's design
  review found in its exemption field.

**Evidence.** A fixture document asserting a post-adoption decision with no
citation fails; with a citation to a real entry passes; with a citation to an
id that does not exist fails. The real tree passes without editing any document
to make it so.

## Stage 3 — label what came before

The seven pre-adoption attributions are marked unverifiable. The three known
bad — 2026-07-28, 2026-09-09, 2026-09-10 — are marked as such, with a pointer
to `ROADMAP.md` §S1.

**Nothing is deleted and nothing is promoted.** This stage adds labels and a
pointer; it does not re-open, re-decide or re-word any of them. Where a
document's attribution is load-bearing for a rule that is still in force, say
so in the review package and leave it to the owner.

## What to return, each stage

A review-request package under `.git-exclude/review-requests/`, in the form the
RFC 102, 103 and design-review packages used: what was built, the evidence
table, the mutations with their results, the gates run on the final tree, and
per-hunk SHA-256 hashes against the stated baseline.
