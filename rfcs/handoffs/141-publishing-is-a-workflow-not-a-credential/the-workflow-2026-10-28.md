# RFC 141 — build the publish workflow

> ## Whose turn it is: **the implementation role's.**
> [RFC 141](../../accepted/141-publishing-is-a-workflow-not-a-credential.md),
> **Accepted and reviewed 2026-10-28**. Cycle H item 3, and the last
> round before `0.86.0`'s preparation.

**Dispatched.** 2026-10-28 JST, by the architect.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

**`0.86.0` does not publish through this.** §7 is settled: the workflow
is built and its refusals exercised here, and **`0.87.0` is the first
release published through it**. Nothing you build this round is used by
the next cut.

## 1 — a correction to the RFC, found before dispatching

RFC 141 §1 names **`rust-lang/crates-io-auth-action@v1`**. **That form
cannot be committed to this repository.** A3.4 condition 1 requires
every `uses:` in a hand-written workflow to be pinned to a **full
40-hex commit SHA**, and condition 2 requires that SHA to appear in
`contracts/gate-inputs.toml`'s `[actions]` table.

So: resolve `crates-io-auth-action`'s v1 tag to its commit SHA, pin it,
and add it to `[actions]` with a key in the existing style
(`checkout_v6`, `setup_python_v7`, …). `actions/checkout` is already
there — **reuse `checkout_v6`'s SHA rather than adding a second entry**.

**Verify the SHA you pin is the one the tag points at**, and say in
your return how you checked. A pinned SHA that nobody verified is worse
than a tag, because it looks deliberate.

## 2 — the gate, which is the whole point of the round

**§3 of the RFC is why this is not a ten-line workflow.** RFC 131 D2
makes a release cut require every `[gates]` lane green via
`workflow_dispatch` on the exact commit being tagged. A person checks
that today. **A publish workflow that does not check it removes that
person without replacing them**, and the automation would weaken D2
rather than strengthen it.

Before `crates-io-auth-action` runs, establish for the commit the tag
points at:

1. a **`CI` run of event `workflow_dispatch` on that exact SHA**
   concluded `success`, **with 0 skipped jobs** — not merely *a* run on
   that SHA, and not a `push` run;
2. a **`Security audit` run on the same SHA** concluded `success`;
3. the tag is **annotated** and its commit is **on `main`**;
4. the **six `Cargo.toml` version fields equal the tag**.

**Any failure stops the job before a token is minted.** Not a warning,
not a skip.

**Count jobs correctly.** A job still running has `conclusion = null`,
which is *not* `"skipped"` — count `success`, `failure` and `skipped`
explicitly and treat anything else as "not finished". **I got this
wrong this week** reporting a run as complete when four cargo lanes
were still going; do not inherit it.

**A negative fixture per refusal**, four of them. This project does not
add a gate behaviour without one.

## 3 — the trigger

**`workflow_dispatch` taking the tag as an input, in a GitHub
Environment with a required reviewer.** Not a tag push: anyone who can
push a tag would then be able to publish.

`pull_request_target` and `workflow_run` are blocked from Trusted
Publishing by crates.io since January 2026 — do not try to use either.

**Name the workflow file deliberately and say what you chose.** The
filename is part of the trust configuration on crates.io, so renaming
it later breaks publishing rather than silently widening it.

## 4 — the publish itself

Six **explicit sequential steps** in the order
`docs/src/contributing/release-process.md` documents: `sui-id-shared`,
`sui-id-i18n`, `sui-id-store`, `sui-id-core`, `sui-id-web`, `sui-id`.
Not a loop over a list — the order is the safety property, and it
should be readable as six lines.

`cargo publish` waits for a crate to reach the index before returning,
so no hand-tuned sleeps.

**A partial publish must be reported as one.** If step four fails, the
job fails naming **which crates are already on the registry**. That is
the state `cargo yank` cannot undo, and the operator needs it precisely.

## 5 — what stays with a person, and must not be automated

- **The tag.** Still manual.
- **Level B and the `Security audit` reading.** Still the architect's;
  this workflow *verifies* them, it does not obtain them.
- **The six crates.io Trusted Publishing configurations** are
  `@nabbisen`'s — account settings on the registry. **Do not block the
  round on them**: the workflow and all four refusal fixtures can be
  built and tested before they exist.

## 6 — what not to do

- **Do not add a `CARGO_REGISTRY_TOKEN` secret**, or any fallback to
  token auth. The point is removing the standing credential.
- **Do not enable Trusted-Publishing-only mode** or revoke anything —
  §6 of the RFC puts that *after* the first successful workflow
  publish, so a failure of the new path falls back to the old one
  rather than locking the project out of its own registry.
- **Do not make the workflow publish on tag push.**
- **Do not touch `ci.yml`** — it is generated, and this is a separate
  hand-written workflow.
- **Write the `CHANGELOG` entry?** **No** — this is release machinery
  and reaches no operator. Second exception to cycle H's standing
  instruction, same reason as the local-matrix round.
- **No release work.** No tag, no publish, no Level B claim.

## 7 — what to return

- The workflow filename you chose, and why.
- How you verified the pinned SHA matches the tag (§1).
- The four negative fixtures, each with the refusal it proves.
- **`bash scripts/local-matrix.sh`'s output** and the count.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.
- Per test, the mutation and the assertion failure.
- Any public signature change, named in prose. *(None expected.)*
- **Whether A3.4 passed on the first run**, and if not, what it caught
  — it is the gate most likely to have an opinion about a new
  hand-written workflow, and its opinion is the point.

Return it as a review request in `.git-exclude/review-requests/`. Leave
the working tree uncommitted.
