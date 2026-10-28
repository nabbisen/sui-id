# RFC 141 — Publishing is a workflow, not a credential

**Status.** Accepted
**Accepted on.** 2026-10-28
**Approved by.** `@nabbisen`, 2026-10-28: *"RFC 141 is Accepted."* **§7
settled the same day: `0.87.0`** — build the workflow in `0.86.0`,
exercise the refusal fixtures, and let the first real use be a release
that is not already waiting on it.
**Independent design review.** [Design review record, 2026-10-28](../handoffs/141-publishing-is-a-workflow-not-a-credential/design-review-2026-10-28.md).
**No design review has been recorded, and this field says so rather
than implying one.** `ROADMAP.md` §S1a places design review with the
accountable approver named below, or an external architect engaged for
it — never the implementation role. **No external architect was
engaged, and the approver has not stated whether his acceptance
included a review.** An earlier revision of this field asserted that it
did; that was the architect's inference from §S1a, not anything the
approver said, and it is withdrawn. The architect wrote this RFC and
did **not** review it: §S1c permits architect self-review only where it
is asked for, and **it was not asked for here**. **Open — see the
record.**
**Implementation owner.** The mid-capability model (implementation and
testing agent).
**Security review.** Required — this changes how six crates are
authenticated to crates.io, and a workflow that can publish is a
workflow worth attacking.

**Design prerequisites.** None.
**Implementation prerequisites.** None under RFC 000's status rule, which
this RFC now satisfies. Six per-crate configurations on crates.io are
`@nabbisen`'s and are not implementation; see §6. The workflow can be
built and its refusal fixtures exercised before those exist.
**Closure prerequisites.** A tagged release publishes all six crates
through the workflow with no long-lived registry token in the process;
the workflow refuses to publish a commit that does not carry Level B;
Trusted-Publishing-only mode is enabled on all six crates and the
workstation token is revoked; and the first release published this way
is confirmed by `scripts/check-published-versions.py` and a direct
registry query, as every release already is.
**Tracks.** RFC 093 owns the build and release-gate contract; this
extends it to the publish. Arises from `0.85.0`.
**Touches.** `.github/workflows/` (one new workflow),
`docs/src/contributing/release-process.md`, `contracts/gate-inputs.toml`
if the new workflow needs declaring there.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** The architect (high-capability model).

## Summary

Six crates are published today by running `cargo publish` six times
from a workstation, authenticated by a long-lived API token in
`~/.cargo/credentials.toml`. crates.io has supported **Trusted
Publishing** since 2025: a GitHub Actions job exchanges an OIDC identity
for a short-lived token, and since January 2026 a crate can be set to
**Trusted-Publishing-only**, refusing API-token publishes outright. This
RFC moves the publish into a workflow, removes the standing credential,
and makes the publication order code rather than a checklist.

## Background

Three things in this project's own history motivate it, and none is
hypothetical.

**The order is a checklist a human follows.** Before 2026-08-26
`docs/src/contributing/release-process.md` listed five steps and omitted
`sui-id-i18n`, which `sui-id-core` and `sui-id-web` both depend on.
Following it literally would have published `sui-id-shared`, failed at
`sui-id-core`, and left a partial release on the registry — which
`cargo yank` can mark unusable but cannot remove. It was caught by
reading, during the 0.77.0 release.

**Tagged-but-unpublished is a state this project has been in for
months.** `0.78.0` and three `0.76.x` tags were never published; the
first three sat undetected for three months. RFC 131 D7 added
`scripts/check-published-versions.py` as the detector. A publish that
runs from the tag closes the window the detector exists to find.

**The last step is the one a guard stops.** At `0.85.0` every gate was
green, the tag was pushed, and the publish was refused by a permission
guard — correctly, because it is irreversible and outward-facing. The
answer is not to weaken the guard but to move the publish somewhere an
approval gate is the natural shape: a workflow environment with a
required reviewer.

## Design

### 1 — the mechanism

A workflow job with `permissions: id-token: write`, running
`rust-lang/crates-io-auth-action@v1` to mint a short-lived token, then
`cargo publish`. Each of the six crates is configured on crates.io with
the repository, the workflow filename and the environment; the filename
is part of the trust configuration, so renaming the workflow breaks
publishing rather than silently widening it.

**Prerequisite already met:** Trusted Publishing requires a crate's
first release to have been published manually. All six have been on
crates.io since the `0.76` line.

### 2 — the trigger, and why not a bare tag push

Triggering on a tag push alone would mean **anyone who can push a tag
can publish**. The workflow therefore runs on `workflow_dispatch` taking
the tag as an input, in a **GitHub Environment with a required
reviewer**. The publish waits in the Actions UI until approved.

`pull_request_target` and `workflow_run` are blocked from Trusted
Publishing by crates.io since January 2026, which removes two trigger
shapes that would otherwise need arguing about.

### 3 — the gate the workflow must enforce

**It must refuse to publish a commit that does not carry Level B.**
RFC 131 D2 makes a release cut require every `[gates]` lane green via
`workflow_dispatch` on the exact commit being tagged. If the publish
workflow does not check that, automating the publish *weakens* D2 by
removing the human who was checking it.

Concretely, before minting a token the workflow must establish, for the
commit the tag points at:

- a **`CI` `workflow_dispatch` run** on that exact SHA concluded
  `success`, with **0 skipped jobs** — not merely some run on that SHA;
- a **`Security audit`** run on the same SHA concluded `success`;
- the tag is annotated and points at a commit on `main`;
- the six `Cargo.toml` version fields equal the tag.

**Any of these failing stops the job before `crates-io-auth-action`
runs.** A negative fixture must prove each refusal — the project does
not add a gate behaviour without one.

### 4 — the order is code

The six publishes run as explicit sequential steps in the dependency
order `release-process.md` already documents: `sui-id-shared`,
`sui-id-i18n`, `sui-id-store`, `sui-id-core`, `sui-id-web`, `sui-id`.
`cargo publish` now waits for a crate to appear on the index before
returning, so the sequence needs no hand-tuned sleeps.

**A partial publish must be reported as a partial publish.** If step
four fails, the workflow fails loudly naming which crates are already
on the registry, because that is the state `cargo yank` cannot undo and
the operator needs to know it precisely.

### 5 — what stays with a person

**The tag stays manual**, and so does Level B. This RFC automates the
mechanical, irreversible step — not the judgement that a commit is
ready. `release-process.md`'s step 0 and step 5 readings of the
`Security audit` lane remain the architect's.

### 6 — what is `@nabbisen`'s, and cannot be automated

**Six per-crate Trusted Publishing configurations on crates.io**, each
naming this repository, the workflow filename and the environment.
These are account settings on the registry's own site; no part of this
RFC can perform them.

**Then, and only after the first successful workflow publish:** enable
**Trusted-Publishing-only** on all six crates and revoke the workstation
API token. Doing it in that order means a failure of the new path falls
back to the old one instead of locking the project out of its own
registry.

## Alternatives rejected

- **A long-lived `CARGO_REGISTRY_TOKEN` in GitHub Secrets.** Moves the
  credential rather than removing it, and a repository secret readable
  by a workflow is a broader exposure than a file on one machine, not a
  narrower one.
- **Keep publishing from a workstation and script the order locally.**
  Fixes the ordering risk and none of the credential risk, and leaves
  the publish unauditable.
- **Trigger on tag push with no environment.** Simplest, and makes tag
  push equivalent to publish authority. Rejected on that alone.
- **Publish all six in one `cargo publish --workspace` invocation.**
  Attractive, and it does respect dependency order — but it is one
  atomic-looking command whose partial failure is harder to report
  precisely than six named steps. Revisit once §4's reporting has been
  exercised at least once.

## §7 — settled: `0.87.0`

**`@nabbisen`, 2026-10-28: `0.87.0`.** The workflow is built in
`0.86.0` and its refusal fixtures exercised there; **`0.86.0` itself is
published by the existing path**, and `0.87.0` is the first release
published through the workflow. The alternative would have tested it
sooner at the cost of debugging release machinery during a release.
