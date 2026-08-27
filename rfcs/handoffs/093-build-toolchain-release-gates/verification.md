# RFC 093 verification and evidence contract

**Governing RFC:** [RFC 093](../../done/093-build-toolchain-release-gates.md)

This applies to every M1a and M1b package, and is the standard later milestones
inherit. It exists because RFC 093's stated primary threat is gate weakening:
evidence assembled from different commits, a feature hidden outside default
builds, or string presence presented as assurance.

## Package rules

1. **Hash-pin every candidate.** List each file with its mode and SHA-256. This
   is what let Wave 2 be verified cheaply — a reviewer could prove by hash that
   only the two intended files changed.
2. **Describe every change.** The "what changed" section is what a reviewer uses
   to decide where to look. An undescribed hunk defeats the pinning. If a file
   has three hunks, describe three.
3. **Freeze pinned files while review is open.** If you find a defect
   mid-review, withdraw the package and resubmit with new hashes. Never edit in
   place; never commit a pinned artifact while its review is open.
4. **Stage by path.** Never `git add -A`. Concurrent lanes and pending packages
   coexist in one working tree.
5. **Evidence binds to one commit.** Results from different trees cannot be
   assembled into a passing matrix. State the exact commit.

## Every blocking check needs a negative fixture

A gate that has never been observed failing is not known to work. For each
blocking check, supply a fixture that makes it fail, and assert the specific
failure label — not merely a non-zero exit.

**Include count-preserving mutations wherever a check counts anything.** Two
real G12 defects were exactly this shape: a token relocated between theme roots
while the global count stayed at three, and two declarations on one physical
line that a line-based counter saw as one. If your check counts, ask what an
attacker or a careless edit could keep constant.

## Recording a run

Each job records, and each package reports:

- commit SHA, resolved from `git rev-parse HEAD`, asserted equal to the event
  SHA, plus a dirty-tree check adjacent to that assertion;
- runner image identifier;
- `rustc -Vv`, `cargo -V`, and the version of every other tool the lane uses;
- the literal command, its exit status, and start/end timestamps.

## Honesty rules

- **Never claim a result you did not observe.** `actionlint` and `shellcheck`
  are currently unavailable; either install them in CI or state plainly that
  they are outside the contract. Do not leave them as implied checks.
- **Name the environment.** Local results are local. The G12 parser leans on
  GNU awk string semantics; `mawk` would behave differently. Consider asserting
  `awk --version | grep -q GNU` alongside the existing Bash constraint check.
- **A partial result is reported as partial.** No lane may be described as
  passing because it is expected to pass.
- **The legacy audit literal check stays diagnostic-only.** Its job and terminal
  summary must carry the wording RFC 093 requires: string parity proves neither
  emission completeness nor mutation/audit atomicity.

## Milestone closure

**M1a closes** when G01–G09 pass on one clean commit, hosted, with the recorded
versions above, and `scripts/check-gate-inputs.sh` enforces the manifest.

**M1b closes** when G10a, G10b, G11 and G12 pass hosted, RFC integrity reports
no known debt, no permanent allowlist exists, and the broken-link count is zero.

Neither closure is satisfied by local runs alone. G12 is currently implemented
and locally green but **has never run hosted**, and that gap is the reason M1a
and M1b both require hosted evidence.

## Closure review

Independent closure review confirms that the legacy audit diagnostic is not
represented as structural assurance, that no lane result was carried across
commits, and that every negative fixture was observed failing for its intended
reason rather than incidentally.
