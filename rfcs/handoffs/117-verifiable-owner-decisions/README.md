# RFC 117 implementation handoff — verifiable owner decisions

**Governing RFC.** [RFC 117](../../proposed/117-verifiable-owner-decisions.md), **Proposed**.
Nothing here is authorized until it is Accepted.
**Implementer.** Mid-capability model.
**Baseline.** The commit that adds this file, or later.
**Hard prerequisite.** **Open question 1 must be answered and the key must
exist before stage 1 starts.** Every stage below verifies signatures; without a
key there is nothing to verify and the work cannot be tested. Do not begin by
building the gate and stubbing the key — a gate whose only fixture is a stub is
a gate that has never run.

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
| 1 | The ledger format, the pinned allowed-signers file, and the verify gate | RFC Accepted; **the key exists** |
| 2 | The citation rule: a document asserting a post-adoption owner decision must cite an entry | stage 1 |
| 3 | Labelling the seven pre-adoption attributions (D6) | stage 1 |

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
