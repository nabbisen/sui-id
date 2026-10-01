# RFC 130 — security review, 2026-10-01

**By the architect, who authored this RFC, and therefore not independent.** This
document records the author's own judgement. It is not a completed independent
design review and the RFC's header says so. `@nabbisen` approved RFC 130 on
2026-10-01 ("Accepted."); he did not state that he performed a design review of
it, and this review does not claim he did. Carried under `ROADMAP.md` R1's
residual, the same way RFC 124's and RFC 128's were.

## What is actually at risk

RFC 130 makes it possible for a gate not to run. Every security property this
project gates on — RFC 112's fail-closed schema read, RFC 120's consent identity,
RFC 125's chain linkage, RFC 126's hashing boundary, RFC 124's uniform response —
is enforced by a test in G02/G04/G05/G06 or a lint in G07/G07b. A skipped lane is
an unenforced property, and the skip is silent by construction.

So the whole review reduces to one question: **can a change that affects those
lanes land without running them?**

## Finding 1 — the change detector must fail open, and the RFC does not say so

This is the material finding, and it is a gap in the RFC as accepted.

RFC 130's D6 specifies "a first job computes the changed paths and the scoped
lanes condition on its output." It does not specify what that job does when it
cannot determine the changed set. The cases where it cannot are ordinary, not
exotic:

- **`workflow_dispatch`** — there is no base ref to diff against. This is the very
  mechanism D5 relies on for obtaining a complete-matrix green, so a detector that
  returned "nothing changed" here would defeat D5's own escape hatch.
- **A force-push or a rewritten base** — `github.event.before` no longer exists in
  the repository, so the diff fails rather than returning a set.
- **The first push on a new branch** — `before` is the all-zeroes SHA.
- **A merge commit or a multi-parent range**, where a naive two-dot diff can report
  a narrower set than the push actually introduced.

In each, a detector that treats "I could not tell" as "nothing relevant changed"
skips every Rust lane on a change that may be entirely Rust. **That is a silently
unenforced gate, which is precisely the failure class RFC 130's own Risk section
names.**

**Required: the detector fails open.** Any condition under which the changed set
cannot be computed with certainty — absent, unresolvable or zero base ref,
non-`push` event, diff error — runs the complete matrix. Scoping is an
optimisation applied only when the inputs are known; it is never the fallback.
Recorded as D7.

## Finding 2 — the scope is complete today, and that is not the same as correct

I measured D3's list against the tree rather than reasoning about it. The known
categories of file that change what a Rust lane does while living outside
`crates/**` and `Cargo.*`:

| Candidate | Present today |
|---|---|
| `.cargo/config.toml` (build flags, target cfg) | **no** |
| `rust-toolchain.toml` / `rust-toolchain` | **no** — toolchains are pinned in `contracts/gate-inputs.toml`, which D3 covers |
| `clippy.toml` (lint config for G07/G07b) | **no** |
| `rustfmt.toml` / `.rustfmt.toml` (G08) | **no** |
| `deny.toml` | **no** |
| any `build.rs` | **no** |
| `.sqlx/` offline query metadata | **no** |

So D3's list is complete for the tree as it stands, and I verified that rather
than asserting it. But completeness-by-absence is fragile: **any of these appearing
later would fall outside the declared scope and silently narrow a lane's
trigger.** The RFC's closure prerequisite says "no gate's scope is narrower than
the inputs it actually reads," which states the property but assigns it to nobody.

**Required: that property is checked, not reviewed once.** `check-gate-inputs.sh`
(A3.4) gains an assertion that no file matching the candidate set above exists
outside the declared Rust scope, failing with the path and the gate it would
affect. Adding `.cargo/config.toml` should then break CI loudly rather than
quietly shrink G01–G09b's trigger. Recorded as D8.

## What I checked and found no problem with

- **The always-on set (D2) is the right boundary.** G11, G13, G15, G16, G17 and
  G18 are the gates that guard documentary claims, and they are the cheapest jobs
  in the matrix — the measured cost of never scoping them is about two minutes.
  G12, G13 and G17 do read `crates/`, which would make them scoping candidates on
  cost grounds; D2 excludes them by rule, and that is the correct trade.
- **No security property is enforced only by a scoped gate and nothing else.** The
  governance gates that remain always-on are the ones that check what documents
  claim; the scoped lanes check what the code does. A Rust change runs the latter
  by definition of the scope.
- **D4's fail-closed generation** is the right shape: an absent scope is loud. It
  is the generator-time counterpart of Finding 1's run-time requirement, and the
  two together are what make the mechanism safe rather than merely cheap.
- **D5 is correctly left open and correctly made a blocker.** Deciding that a
  release needs no complete-matrix green would be a material weakening, and it is
  not a decision this RFC, or I, should make.

## Verdict

**Accept, with D7 and D8 added as required amendments.** The design is sound and
the measurement behind it is solid, but as accepted it specifies the optimisation
without specifying the two properties that keep it from becoming a silent hole.
Both are small; neither changes the design's shape.

Implementation remains blocked on D5 regardless, which is `@nabbisen`'s to settle.
