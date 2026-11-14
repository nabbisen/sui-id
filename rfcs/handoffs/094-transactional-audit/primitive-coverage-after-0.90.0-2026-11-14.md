# The write primitives with no direct test — after `0.90.0`

> ## Landed 2026-11-14, as `0.91.0` S2, accepted as returned —
> `.git-exclude/reviewed/primitive-coverage-after-0.90.0-2026-11-14.md`.
>
> ## Whose turn it was: **the implementation role's.**
> Return it as a **working tree, not a commit.** It lands after the `0.90.0`
> tag, so the tagged tree stays the one the owner's live run tested.

**Dispatched.** 2026-11-14, by the architect, under the owner's go-ahead
of the same day.
**Baseline.** `c218a7e` (record `git log -1` when you start).
**Where this came from.** `0.90.0`'s preparation listed the
`contracts/write-commands.toml` rows whose primitives no test calls
directly. The review of that package
(`.git-exclude/reviewed/release-0.90.0-preparation-2026-11-12.md` §3) left
them as this follow-up.
**Release.** The next minor, `0.91.0`, not a `0.90.x` patch: §A removes
one public function from `sui-id-store`, a published crate, and gates
another.

The architect measured every claim below on `c218a7e`. Re-measure anything
you rely on before you change it, and say in the return if one is wrong.

## A — two primitives no production code calls: remove them from production, not test them

A test would prove that code nothing runs still works. The honest fix is
the one `0.90.0` applied to `refresh_tokens::insert`
(`crates/sui-id-store/src/repos/refresh_tokens.rs`, now gated
`#[cfg(any(test, feature = "test-support"))]`).

1. **`users::set_last_login`** (`crates/sui-id-store/src/repos/users.rs:878`)
   has **no caller at all**, test or production (`grep -rnE
   '\bset_last_login\(' crates xtask` finds only the definition). Its doc
   comment says *"Called by the session-creation path"*, which is no longer
   true. `last_login_at` is written inside the sign-in commands'
   transactions: `record_password_login_within_tx` (`users.rs:377`),
   `record_second_factor_login_within_tx`, and `record_federated_login_within_tx`
   (`users.rs:1035`), each named on its own L row.
   - **Delete the function.**
   - Correct **U24**'s `mutation_surface` (`write-commands.toml`, the row
     with `id = "U24"`) to name only `users::clear_lockout`. Correct its
     `test_id` so it no longer mentions `set_last_login`.
   - Grep the descriptions too, not only the code: any doc, comment or
     contract prose that says `set_last_login` records the login time is
     now false (`grep -rn set_last_login .`, excluding `target/`).
2. **`signing_keys::insert_with_plaintext`**
   (`crates/sui-id-store/src/repos/signing_keys.rs:29`) has **only test
   callers**: five, all under `crates/sui-id-store/src/commands/tests/runner/`
   (`master_key.rs`, `setup.rs`, `key_rotation.rs`).
   - Gate it `#[cfg(any(test, feature = "test-support"))]`, with a doc line
     saying it is test-only and naming the production insert path that K06's
     row already covers (`insert_sealed_on_conn`).
   - Confirm that every one of the five callers still compiles under the
     gate, in both `cargo test` and the `--all-features` build.
   - Correct **K06**'s `mutation_surface` and `test_id` to drop it.

G23 (`cargo xtask audit-structure`) is the check that a write function
no row names must not exist. Run it after each of the two, and quote what
it prints.

## B — one test per primitive, each with a boundary its WHERE clause decides

**For each primitive below, write a direct test** in the store crate's
sibling test file for that repo (RFC 137; follow the existing
`repos/<name>/tests.rs` layout, or create it the same way). Each test must:

- **seed** at least one row the statement must change **and** one it must
  not, so an assertion of "unchanged" cannot pass on an empty table;
- **assert the changed columns' values**, not only the returned count;
- **name, in the return, the mutation that turns it red:** delete or
  weaken one clause of the WHERE, or one SET column, and record that the
  test failed. One mutation per clause.

| Row | Primitive | Measured statement (on `c218a7e`) | The row it must leave alone |
|---|---|---|---|
| **T08** | `refresh_tokens::backfill_token_hashes` (`refresh_tokens.rs:565`) | `SELECT id, token_enc FROM refresh_tokens WHERE token_hash IS NULL`, then a hash write per row (read the body) | a row whose `token_hash` is already set. Also run it **twice**: the second run returns 0 and changes nothing |
| **T06** | `refresh_tokens::revoke_all_for_user` (`refresh_tokens.rs:172`) | `UPDATE refresh_tokens SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL` | another user's live token, **and** this user's already-revoked token, whose `revoked_at` must keep its old value |
| **U29** | `user_webauthn_credentials::update_passkey` (`user_webauthn_credentials.rs:224`) | `UPDATE user_webauthn_credentials SET passkey_enc = ?1, last_used_at = ?2 WHERE id = ?3` | a second credential of the **same user** |
| **U36** | `user_consent::touch_last_used` (`user_consent.rs:107`) | `UPDATE user_consent SET last_used_at = ?3 WHERE user_id = ?1 AND client_id = ?2` | same user, other client; **and** same client, other user (one per clause) |
| **O02** | `email_outbox::mark_permanently_failed` (`email_outbox.rs:160`) | `UPDATE email_outbox SET state = 'failed', attempt_count = attempt_count + 1, … WHERE id = ?3` (read the elided SET columns and assert each) | another queued row |
| **O03** | `login_pending_mfa::purge_expired` (`login_pending_mfa.rs:126`) | `DELETE FROM login_pending_mfa WHERE expires_at < ?1` | an unexpired row |
| **O03** | `webauthn_pending::purge_expired` (`webauthn_pending.rs:121`) | `DELETE FROM webauthn_pending WHERE expires_at < ?1` | an unexpired row |
| **O03** | `revoked_access_tokens::purge_expired` (`revoked_access_tokens.rs:54`) | `DELETE FROM revoked_access_tokens WHERE exp < ?1` | an unexpired row |
| **U32** | `login_pending_mfa::delete` (`login_pending_mfa.rs:111`) | read the body | another pending row |
| **U33** | `webauthn_pending::delete` (`webauthn_pending.rs:81`) | read the body | another pending row |

**These read the wall clock, not `SharedClock`.** `revoke_all_for_user`
and `update_passkey` bind `Utc::now()`. The three `purge_expired`
functions take no clock either, so read each body for what binds `?1`. A
test cannot set their "now". So:
- seed an expired row **days** in the past and a live row **days** in the
  future, never seconds either side of now;
- assert a written timestamp as "between the instants read just before
  and just after the call", never equal to a value you chose. Read each
  instant **once** into a variable.

**Then update each row's `test_id`** to name its new test, backticked so
G23's check (H) resolves it. Remove the "no direct test" wording only from
the rows whose every named primitive now has one.

## C — the record

- `CHANGELOG.md` `## [Unreleased]`, under `### Changed`: one entry saying
  that `sui-id-store` no longer exports `users::set_last_login`, which
  nothing called, and that `signing_keys::insert_with_plaintext` is now
  `test-support` only. No entry for the tests themselves.
- **Do not touch** `## [0.90.0]`, the version numbers, or anything under
  `rfcs/` other than this file's return. Leave the ROADMAP to the architect.

## D — the gates and the return

- `cargo fmt --all --check`, then **`bash scripts/local-matrix.sh`, the
  full matrix** (the `+stable` lanes included, which CI skips on push), on
  the tree you return, with no edits during the run. Quote its last three
  lines.
- The return goes in
  `.git-exclude/review-requests/primitive-coverage-after-0.90.0-<date>.md`,
  with a hunk-hash file beside it, as usual. For each test, give the
  mutation and the observed failure. List every row whose `mutation_surface`
  or `test_id` changed, quoted before and after.
- **If a primitive can't be tested directly,** say why, measured, and cite
  the indirect test that asserts its write instead, as U28's row now does.
  "It can't be tested" is usually false, so try first.
