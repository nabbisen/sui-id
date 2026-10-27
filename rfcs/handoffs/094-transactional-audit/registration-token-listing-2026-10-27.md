# `0.85.0` — one more round before the cut: registration tokens must be findable

> ## Whose turn it is: **the implementation role's.**
> **`@nabbisen`, 2026-10-27: _"Your recommendation is authorized."_** The
> cut waits for this round. It is small.

**Dispatched.** 2026-10-27 JST, by the architect.
**Governing RFC.** RFC 094, **Accepted 2026-08-27** — this arises from
M2b wave 4 stage 4's C22 and finishes it.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 1 — why this round exists

Stage 4 added `sui-id admin revoke-registration-token --id ID`, and
**nothing tells an operator what an ID is.** Measured on `13c9781`:

- `run_admin_issue_registration_token` (`cli.rs:1052-1082`) prints the
  token and a `curl` example. **It never prints the row's id**, though
  it generates one at `:977`.
- There is **no list subcommand**, no admin page, no other read surface
  — `git grep client_registration_token::` outside the repo module finds
  only `create` and the new revoke.
- So the only way to obtain an id is opening the SQLite database.

A command that takes `--id` implies ids are obtainable. This one's are
not, which makes a working remedy unusable from the CLI and, worse,
makes it *look* usable. That is the gap this round closes.

**It is cheap because the store half already exists.**
`repos::client_registration_token::list()` (`:78`) is written, returns
`Vec<RegistrationTokenRow>`, and **has no caller**. This is a read, so
there is no sealed command, no migration and no audit event.

## 2 — what to build

### 2a — `sui-id admin list-registration-tokens [--config PATH]`

Wraps the existing `list()`. One row per token, newest first. Columns:

- the **id**, which is the whole point;
- `created_at`;
- `max_uses` and `used_count` — render `max_uses = 0` as `unlimited`,
  since that is what the column means (`:22`'s own comment) and `0`
  reads as the opposite;
- `expires_at`, rendered as `never` when `None`, which is what the issue
  command always writes today;
- whether it is **revoked**, and when.

**Never print the token or its hash.** The row holds `token_hash`; it
must not reach stdout. The raw token is shown once at issue and never
again — that property is deliberate and this command must not weaken it.

Register it in the subcommand list (`cli.rs:35-36`'s array), the
dispatch `match` (`:164-167`), and **both** help texts — the usage block
near `:697-701` and the subaction list near `:760`. RFC 107 exists
because two dispatched subactions were once missing from `--help`; do
not make it three.

### 2b — `issue-registration-token` prints the id

Add the id to that command's output, labelled, next to the token. An
operator who records the output can then revoke later without this
listing command or the database.

Keep the existing output's shape and its "shown ONCE" warning. The id is
not a secret; the token is. Make it obvious which is which — the point
of this round is that an operator should never have to guess what to
pass to `--id`.

### 2c — the `CHANGELOG`, which you *do* write this round

**A change from the last four rounds, and the change is mine.** Every
wave-4 dispatch told you to write no `CHANGELOG` entry, and the result
was that `0.85.0`'s notes did not exist at cut time and I wrote them all
at the end. That was my error, not yours.

So: **add an `Added` entry for the listing command**, in
`[Unreleased]`'s existing `### Added` section, beside the
`revoke-registration-token` line already there. Mention that
`issue-registration-token` now prints the id. Plain feature prose — no
severity, no `Security` section.

**Do not touch anything else in `[Unreleased]`**, and in particular not
the three severity paragraphs or the summary.

## 3 — tests

- **The listing shows a token's id and its revoked state.** Issue two,
  revoke one, assert both appear with the right states and the right
  ids.
- **Neither the token nor its hash appears in the output.** Assert the
  rendered string contains neither. **Mutate it**: print the hash,
  confirm the test fails.
- **`max_uses = 0` renders as `unlimited`**, not `0`.
- **`expires_at = None` renders as `never`.**
- **`issue-registration-token`'s output contains the id.** Mutate: drop
  it, confirm failure.

Per test, report the mutation and the assertion failure it produced, as
the last four rounds did.

## 4 — what not to do

- **Do not add an expiry to `issue-registration-token`.** Still out of
  scope, still a product decision.
- **Do not print the token or its hash** (§2a).
- **Do not add an audit event.** Listing is a read.
- **Do not touch the three severity paragraphs or the summary** in
  `[Unreleased]`.
- **No release work.** No tag, no publish, no `workflow_dispatch`, no
  Level B claim, and do not title the gate summary "Level B".

## 5 — your local matrix

**Derive the unittest list, do not transcribe it** — stage 4's own came
back thirteen when there are fourteen:

```
grep -o "python3.14 -m unittest scripts\.tests\.test_[a-z_0-9]*" \
  .github/workflows/ci.yml | sed 's/.*tests\.//' | sort -u
```

**Run the Rust store tests before them**, or
`test_check_m2a_rollback_coverage` reports a false failure from a stale
`target/rfc094-rollback-coverage.txt`.

## 6 — what to return

- The rendered output of both commands, verbatim, for a token that is
  live and one that is revoked.
- The full local matrix, **as the local matrix**, with the `+stable`
  caveat and the derived unittest list you ran.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.
- Per test, the mutation and the assertion failure.
- Any public signature change, named in prose.
- G20's count, which should not move — nothing here is a write command.

Return it as a review request in `.git-exclude/review-requests/`. Leave
the working tree uncommitted.
