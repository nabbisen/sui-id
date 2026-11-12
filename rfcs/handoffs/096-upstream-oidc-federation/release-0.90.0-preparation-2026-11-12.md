# `0.90.0` — release preparation

> ## Whose turn it is: **the implementation role's.**
> §A is one product fix, and §B is the contract cleanup decided on the
> completion assessment. §C is the mechanical half of the release. The tag, the publish,
> the `Security audit` readings, the live run (stage 14) and **the RFC and
> ROADMAP records** stay with the architect and the owner.

**Dispatched.** 2026-11-12, by the architect.
**What this release is.** Cycle L: RFC 096-B2, federation administration,
**accepted 2026-11-12** —
[`096-b2-completion-assessment-2026-11-12.md`](../096-upstream-oidc-federation/096-b2-completion-assessment-2026-11-12.md).
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
**Baseline.** The tip of `main` when you start (`git log -1`; record it).
**Step 0.** The scheduled `Security audit` on 2026-11-11 had failed on
RUSTSEC-2026-0486 (`rustix` 1.1.5, unsound), and so did the architect's run
`43239200566` on `1b0b4c9`, before the fix. **Fixed in `d5a0b2a`** (`rustix`
1.1.6, with a `### Security` entry). `Security audit` on `d5a0b2a`: **success**,
run `43244807209`. Nothing else to carry in.

**After this lands**, the owner runs 096-B1's stage 14 live run on **this
stage's commit** (`rfcs/handoffs/096-upstream-oidc-federation/096-b1-stage14-live-run-runbook-2026-11-12.md`).
Then any fixes, then the tag. So no tag-shaped work happens here.

## A — one fix before the cut: where a federated sign-in lands

**The defect.** With no `next`, all three federated success paths land on
`/admin` (`handlers/federation.rs`, `post_signin_target`; called at `:1434`,
`:1762`, and `handlers/admin/auth.rs:843`). A user provisioned by
`provision_on_first_login` is a plain `user`. So **a new user who presses
the login page's "Sign in with …" button signs in successfully, and then
gets "Forbidden"** from `/admin`'s `CurrentAdminOrAuditor`. The password
path never does this: it decides the audience before signing in (RFC 102
A7, `handlers/admin/auth.rs:474-482`).

**The fix.** When there is no valid `next`, land by role:
- `/admin` for a user who `can_read_admin()` (`sui-id-core/src/authn/session.rs:242`'s
  rule);
- otherwise `/me/security`, the self-service page every user may open.

**A valid `next` still wins**, which is the S5b fix. Give
`post_signin_target` the user's role, or the default, and keep one helper
for all three paths.

**Tests**, one per path, because each path has its own call site:
- first-login provisioning, no `next` → `/me/security`;
- existing link, a non-admin user → `/me/security`;
- existing link, an admin → `/admin`;
- the second-factor completion, a non-admin → `/me/security`;
- and `next` still wins on each.

**Mutation:** drop the role check, and each non-admin test goes red.

## B — every `test_id` names a real test (the completion assessment's §5.3)

**The problem.** 46 `test_id` entries in `contracts/write-commands.toml`
name no function. The completion assessment §4 lists the shape:
`a_u01_create_normal`, `a_k01_signing_rotate`, and their kin. No gate checks
that a `test_id` resolves, which is how 096-B2's three slipped through.

1. **Find each command's real tests through the coverage registry, not by
   guessing.** The tests that call `record_exactly_once_coverage("<id>")` and
   `record_rollback_coverage("<id>")` are that command's evidence. Replace
   each unresolved label with those tests' names.
2. **If a command has no test registered, do not cite a nearby test.**
   Report it in the return, as a missing test. A label that named nothing is
   not repaired by naming something that does not test it.
3. **The gate.** `cargo xtask audit-structure` (G23) fails when a
   backticked identifier in a `test_id` is not the name of a function marked
   `#[test]` or `#[tokio::test]` somewhere in `crates/` or `xtask/`. Rows
   whose `test_id` says they are reserved (`target-absent`) are exempt, as
   they are today. Add a **negative fixture** in G23's own fixture set
   (`--negative-fixtures`): a row naming a function that does not exist must
   be refused.
4. A `test_id` lists tests only: **no mutation descriptions.** 096-B2's rows
   had two.

## C — the mechanical half

### C.1 — the version bump

**Six sites** in the root `Cargo.toml`: `:21` `version`, and `:34-38`, the five
internal crates. Each `0.89.0` → `0.90.0`. Let cargo refresh `Cargo.lock`,
and confirm only the workspace members moved (the six, plus `xtask`). The
Quick Start's `VERSION=` (`quick-start.md:27`) → `0.90.0`.

### C.2 — the two pinned documents

The same arithmetic as 0.89.0's preparation, with `tolerance_minor = 2`.
That covers `docs/development-specification.md`'s banner and
`docs/threat-model.md:4`. Show the arithmetic.

### C.3 — the upgrade note

S7 wrote `### v0.90.x`. **Add what it does not yet say:**
- **Two migrations run** (0050 and 0051) from 0.89.x.
- So **the pre-migration snapshot is taken automatically** before they run.
  Say where to find it, linking to the guide's own section.
- **Going back to 0.89.x means restoring that snapshot.** Link to the
  guide's rollback section, restating nothing.

### C.4 — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.90.0] — <the cut's date>`, with a fresh empty
`[Unreleased]` above. **Three subsections exist: `Added`, `Changed`,
`Security`.** Do not reorder or rewrite them; S7 already ordered them.
**The one-line summary names every subsection**, leading with what the
release is:
1. federation providers are administered from the admin UI, and federated
   sign-in has a button on the login page;
2. then the upgrade consequence (federated sign-in stops until each provider
   is checked and re-enabled);
3. then the `rustix` security update.

Render the Release body:
`python3.14 scripts/extract-changelog-section.py --changelog CHANGELOG.md
--version 0.90.0 --repo nabbisen/sui-id --ref main --root .`. Quote the
output; it must exit 0. Confirm `grep -c '^\[0\.' CHANGELOG.md` is still `0`.

### C.5 — packaging

`cargo package -p sui-id-shared --allow-dirty`, with its output verbatim.

## D — what you must not do

- **No tag**, no publish, no `workflow_dispatch` of anything.
- **Do not touch `rfcs/`, `ROADMAP.md`, or `contracts/owner-attribution-baseline.txt`.**
- **No Level B claim.**
- **Do not restate a gate's command** in any document (RFC 131 D4).

## E — what to return

`.git-exclude/review-requests/release-0.90.0-preparation-2026-11-12.md`, with:
- the baseline;
- §A's change, its tests, and the mutation;
- §B's table: each row, the old label, and the tests it now names. **Every
  command with no registered test, listed separately.** Also the gate's
  check and its negative fixture;
- the version sites and the `Cargo.lock` diff;
- the pinned-document arithmetic;
- the upgrade note's added text;
- the promoted `CHANGELOG`, its summary, and **the rendered Release body**;
- `cargo package`'s output;
- `bash scripts/local-matrix.sh` (**full**, not `--changed`), with its count,
  and the names of any failing checks;
- per-hunk hashes, beside the package.
