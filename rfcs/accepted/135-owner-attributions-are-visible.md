# RFC 135 — Owner attributions are visible

**Status.** Accepted
**Accepted on.** 2026-10-05
**Approved by.** `@nabbisen`, 2026-10-05: "Accepted." — settling that RFC 117's unbuilt signing-key half is disposed of and G16 is kept and re-homed here. His ruling the same day was that RFC 117 "does not match the project reality (the team organization etc.) at all. Dispose it if no concern and risk"; the concern found was that disposing of it wholesale would orphan a live gate, and this RFC is how it is disposed of without that.
**Security review.** Not required — this RFC adds no application behaviour and touches no crate. It re-homes an existing governance gate and narrows the design it came from. The exemption reason stands or falls with this RFC's acceptance.

**Design prerequisites.** None.
**Implementation prerequisites.** This RFC Accepted. **No new code** — G16 already exists and passes; the work is re-pointing its ownership and archiving RFC 117.
**Closure prerequisites.** `contracts/gate-inputs.toml` records this RFC as G16's owning RFC; this RFC carries the Gate Matrix heading A3.4 condition 7 requires; RFC 117 is archived as superseded with no header in `proposed/`, `accepted/` or `done/` citing it as authority; and G16's own files cite this RFC rather than RFC 117.
**Tracks.** Governance integrity.
**Touches.** `contracts/gate-inputs.toml`, `contracts/owner-attributions.toml`, `contracts/owner-attribution-baseline.txt`, `scripts/check-owner-attributions.py` (comments only), `rfcs/archive/`, `rfcs/README.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.

## Summary

RFC 117 proposed that owner decisions carry a signature from a hardware key no
agent can use. **`@nabbisen` ruled on 2026-10-05 that this does not match the
project's reality and should be disposed of.** He is right, and the reasons are
specific rather than general. But RFC 117's first stage **already shipped** as
G16, it works, and disposing of the RFC without re-homing the gate would orphan
a live control and break two mechanical checks. This RFC keeps what shipped,
describes it honestly, and lets RFC 117 be archived.

## Why RFC 117's unbuilt half does not fit

**Its threat model is not this project's.** RFC 117 names adversary (a) — *"an
agent writing an attribution it believed"* — as the real one, and records that
all three historical failures were exactly that. D1 and D2, the signing key and
per-decision signatures, defend against a *different* adversary: an agent that
knowingly fabricates and will edit the gate to hide it. **That adversary has
never appeared here, and the design admits the controls against it rest on
repository configuration the agents' credential can already change.**

**Its ceremony does not fit how decisions are actually made.** D2 requires each
ledger entry to carry an individual signature. On 2026-10-05 alone `@nabbisen`
ruled on six separate items in conversation. A hardware touch per ruling would
make the record of a decision more expensive than the decision, and the
predictable result is that rulings stop being recorded rather than start being
signed.

**It was written for a team that does not exist.** RFC 117 dates from the period
whose role names were `codex-developer`, `codex-project-architect` and
`codex-independent-architecture-security-reviewer` — three vendor agent
identities. This team is a human owner, a high-capability model and a
mid-capability model (`.git-exclude/roles/`). The same mismatch produced the
`codex-developer` references corrected on 2026-10-05.

## What G16 actually is, stated honestly

**G16 is a speed bump, not a proof.** It scans Markdown and code comments for
any sentence pairing the owner with a decision verb, and fails on one not in a
closed baseline. Its value is that **the writer is forced to stop and ask
whether a sentence records the owner's words or the writer's claim.**

**Measured: it caught four of the architect's own sentences in the week to
2026-10-05** — a `Status:` line reading "`@nabbisen`'s to rule on", a note about
the gate that tripped the gate, a hypothetical about a scope change, and a
prerequisite framing. None was a fabrication. **Each was a sentence that read
like a decision and was not one**, and the gate is what made that visible.

**It is not evidence of anything.** The script's own docstring says so: *"It does
not verify that an owner decision is real."* RFC 117 proposed signatures to close
that gap. **This RFC accepts the gap instead**, on the grounds above.

## Decisions

**D1 — G16 is owned by this RFC.** `contracts/gate-inputs.toml`'s
`[gate_owners]` records `G16 = "135"`, and this RFC carries the Gate Matrix
heading A3.4 condition 7 requires. No change to what the gate does.

**D2 — RFC 117 is archived as superseded by this RFC**, not deleted. Its
analysis of why git cannot settle an attribution — 411 commits under a shared
signing key, the agents' credential holding `admin: true` with no branch
protection — is accurate and worth keeping as a record of what was measured.

**D3 — The baseline's growth is accepted, and named.** RFC 117's D0 defined the
baseline as *closed*, with new decisions citing a signed ledger entry instead.
There is no ledger and now will not be, so every new attribution goes into the
baseline: **455 lines at adoption on 2026-09-25, 671 on 2026-10-05, +47% in ten
days.**

**This is a real and permanent weakening and it is recorded, not hidden.** A
reviewer scanning a growing file does a harder job than one checking a
signature. The mitigation is the baseline file's own rule — *"Do not add a line
for an attribution you cannot point to the owner's own words for"* — which is a
discipline, not a control, and depends on the architect's good faith.

**D3a — This RFC added nine lines to the baseline, which is the drift it
describes.** Writing a document *about* the attribution gate trips the gate on
nearly every paragraph: two of the nine are `@nabbisen`'s actual words, one is
the required `Accountable owner and approver` field, and **six are this RFC
quoting the gate, the script's docstring, or the baseline file's own rule**. All
nine are legitimate and all nine were baselined. **The demonstration is the
point:** a control whose baseline grows when you document the control is one
whose growth is not, by itself, a signal. That is why D3 accepts the drift
rather than proposing a volume check for it — see the open question.

**D4 — No residual claim of verifiability.** `ROADMAP.md` §S1's R1 residual
keeps carrying review-independence risk. Nothing in the repository may say owner
decisions are *verifiable*; they are *visible*.

## Gate Matrix lane owned by RFC 135

Registered through the multi-source lane registry (RFC 094 R10), as RFC 098,
116 and 134 do. The heading above is the recorded source heading and is matched
by plain equality; do not rename it without changing the manifest in the same
commit. Column layout mirrors RFC 093's table so one parser reads both. This
lane is not new — it transfers from RFC 117, which is archived.

| ID | Toolchain | Features | Blocking command / assertion |
|---|---|---|---|
| G16 | Python 3.14 | n/a | `python3.14 scripts/check-owner-attributions.py --root . --policy contracts/owner-attributions.toml` |

## Open questions

1. **Is accepting D3's weakening right?** The alternative is some cheaper
   freshness check on the baseline — for instance, failing when it grows by more
   than N lines in one commit without an explicit marker. I am **not**
   proposing it: it would catch volume, not falsity, and add a second
   mechanism to maintain. Recorded because accepting a 47% drift silently would
   be the wrong way to decide it.
