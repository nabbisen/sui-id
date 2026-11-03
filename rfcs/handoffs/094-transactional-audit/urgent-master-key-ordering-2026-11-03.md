# Urgent — persist the new master key before the re-seal commits

> ## Whose turn it is: **the implementation role's.**
> **Ahead of cycle J stage 3, not part of it.** This is a
> permanent-data-loss window in a shipped release. The audit conversion
> for the same command waits; this does not.

**Dispatched.** 2026-11-03, by the architect.
**Found by.** Cycle J stage 2's measurement, answering a question about
atomicity — the loss window was not what it was sent to look for.
**Scope.** `sui-id admin rotate-key` only. **No audit work, no
sealed command, no new event** — those are stage 3.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 1 — the window, and why it is not a documentation problem

Today's order (`crates/sui-id/src/cli.rs`):

1. `:435` — `MasterKey::generate()` when `--new-key` is absent. **In
   memory. Never printed** — the summary reports row counts.
2. `:441` — `rotate_master_key(&db, &new_key).await?` — six tables
   re-sealed in one transaction. **This commits.**
3. `:464` — `std::fs::rename(old_path, .bak.<stamp>)?`
4. `:471` — `std::fs::write(old_path, new_key_b64)?`

**The only copy of the key is written at step 4, after step 2 has
already re-sealed the database with it.** If step 3 or 4 fails — disk
full, read-only mount, permissions, a full inode table — the process
exits and the key is gone. Signing keys, refresh tokens, TOTP secrets
and recovery codes, WebAuthn credentials, SMTP configuration and the
email outbox are then **undecryptable by anyone, permanently.**

`docs/src/guides/operators.md` promised *"There is no half-rotated
state to recover from"* until 2026-11-03. **The guide is corrected and
`--new-key` is documented as the safe procedure** — that protects an
operator who reads it. This round protects the one who does not.

## 2 — the fix: make the key durable before the irreversible step

**New order:**

1. Obtain the new key (generate, or read `--new-key`).
2. **Write it to a sibling path — `<configured>.new` — created with
   mode `0600` *atomically*.** Not create-then-`chmod`: use
   `OpenOptions` with `.mode(0o600).create_new(true)` on Unix, so the
   key is never briefly world-readable.
3. Re-seal (`rotate_master_key`). This commits.
4. `rename(configured → .bak.<stamp>)`.
5. `rename(.new → configured)`.

**Why this closes it.** After step 2 the key is on disk before anything
irreversible happens. Steps 4 and 5 are **renames within one
directory**, which allocate nothing and are far less likely to fail
than a write — and if either does, the key is still at `<path>.new`
and the operator can finish by hand.

**`create_new(true)` is not a detail, it is the second half of the
fix.** Consider a previous run that crashed between steps 3 and 5: the
database is on key K₁ and `.new` holds K₁. If a rerun **overwrites**
`.new` with a freshly generated K₂, **K₁ is destroyed and the database
is sealed with it** — the rerun would cause exactly the loss this round
exists to prevent, and it would look like a retry. So:

> **If `<path>.new` already exists, refuse before touching the
> database**, and say why: a previous rotation may not have completed,
> that file may be the only copy of the key the database is already
> using, and the operator must check it against the database before
> removing it.

**On success, remove `.new`** only by the step-5 rename, never by an
unlink that could run after a failure.

**Keep `--new-key` working unchanged.** Still stage through `.new`, so
recovery is uniform and the guide's procedure stays true.

## 3 — what to establish before building, and report either way

- **Can a re-seal failure be injected** so a test can assert that
  `<path>.new` exists *and* the database still opens with the **old**
  key? That is the assertion that proves the key is persisted first.
  **If it cannot be injected, say so and propose the nearest honest
  test** rather than asserting the ordering in a comment.
- **Are `<configured>` and `<configured>.new` guaranteed to be on one
  filesystem?** They share a directory, so a rename should not cross a
  mount — confirm, because the fix rests on it.

## 4 — the tests this needs

1. **Successful rotation**: the key is at the configured path, `.new`
   is gone, the old key is at `.bak.<stamp>`, and the database opens
   with the new key and not the old. (`tests/e2e/key_rotation.rs` has
   the harness.)
2. **`.new` already exists**: the command refuses, and **the database
   still opens with the old key** — assert the refusal *and* that
   nothing was touched.
3. **Mode `0600` at creation**, asserted on the file the command wrote.
4. **§3's ordering test**, if injection is available.

**Mutation evidence**: revert step 2 to today's write-after-reseal and
show test 1 or 4 failing. A fix for an ordering bug that no test
notices when the order is restored has not been shown to close
anything.

## 5 — what not to do

- **No audit work.** Not the sealed command, not a new event, not the
  `let _ = audit::append` swallowing its own error — all stage 3.
- **Do not change what is re-sealed**, or the transaction itself.
- **Do not print the key.** It is a master key; the staged file is the
  durable copy, and stdout is not.
- **Do not remove or rewrite the `.bak` archive step.**
- **No release work.**

## 6 — `CHANGELOG`

**One entry, under `[Unreleased]`, and it is operator-facing and
blunt.** Rotating the master key could lose it permanently if the disk
filled at the wrong moment, taking every encrypted column with it; the
key is now written to disk before the re-seal, so a failure afterwards
is recoverable. **Name the version range affected** — rotation has
behaved this way since the command existed; establish from when and say
so rather than implying it is new.

## 7 — what to return

- The usual package, against the baseline you recorded.
- **§3's two answers**, with `file:line`.
- The four tests, and the mutation.
- `bash scripts/local-matrix.sh`'s output and count.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.

Return it as a review request in `.git-exclude/review-requests/`. Leave
the working tree uncommitted.
