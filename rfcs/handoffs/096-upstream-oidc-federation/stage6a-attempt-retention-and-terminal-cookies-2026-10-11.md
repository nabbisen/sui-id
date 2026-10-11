# RFC 096-B1 stage 6a — attempt retention, and the cookie on every terminal path

**Dispatched.** 2026-10-11 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
amended and authorized 2026-10-11. This is the part of its stage 6 that depends
on nothing open.
**Prior.** Stages 0–5 complete. Stage 5 landed `8a8dec4`; Level B on CI run
`38099337994`.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

## Why this is 6a and not stage 6

Stage 6 is F05, F01 and these two retention items. **F05 and F01 are held**:
both specify a post-commit Class-B `emit_must_attempt` event, and that API does
not exist — `registry.rs:118-124` records `AuditClass::MustAttempt` as *"not
implemented in this Stage-1 slice"*, nothing uses it, and
`commands/tests.rs:162` asserts that no descriptor does. The decision request
is [`class-b-emission-and-f01-class-2026-10-11.md`](class-b-emission-and-f01-class-2026-10-11.md).

**These two items need no command, no event and no registry row.** They are the
whole of this stage, and one of them closes something that is growing in `main`
right now.

## Item 1 — `:604-605`'s retention. Attempt rows are never deleted

RFC 096 `:604-605`: *"Completed/failed/expired attempts are periodically deleted
after 24 hours."*

Measured: `crates/sui-id-store/src/repos/federation_login_attempt.rs` has
exactly three public functions — `insert`, `find_by_state_sha256`, `claim` —
and **no delete of any kind**. `crates/sui-id/src/runtime/gc.rs` runs every 15
minutes (`GC_INTERVAL`, `:18`) and purges six tables: `auth_codes`, `sessions`,
`refresh_tokens`, `login_pending_mfa`, `webauthn_pending`,
`revoked_access_tokens`. `federation_login_attempt` is not one of them.

So every `GET /auth/federated/{slug}/start` inserts a row that is never
removed, on an unauthenticated endpoint, and has done since stage 2. **No
released version is affected** — stages 1 and 2 landed after `0.79.0` — and the
authorized release checkpoint makes this a precondition of `0.80.0`.

**What to build.** `federation_login_attempt::purge_expired`, in the shape the
existing six already use — a plain repo function returning
`StoreResult<usize>`, not a declared write command. Follow
`repos/sessions.rs:177` or `repos/webauthn_pending.rs:121` for the signature
convention, including whether `now` is a parameter or read inside: **match
whichever the neighbours do**, and say which you matched and why.

**The predicate is yours to derive and to justify, not mine to assert.** The
table has `created_at`, `expires_at`, `status` and `claimed_at` and **no
terminal-transition timestamp**, so "24 hours after it became terminal" is not
directly expressible. `expires_at = created_at + 600` for every row, which
makes `expires_at <= now - 24h` a uniform rule covering completed, failed and
expired alike — but derive it against the schema yourself and state the rule
you chose, what it deletes, and what it deliberately does not. In particular
say what happens to a row still `pending` or `exchanging` 24 hours later: it is
expired by `:587-588`'s own definition, so leaving it is not an option, but
whether your predicate catches it is exactly the kind of thing a reader should
not have to infer from SQL.

Register it in `gc.rs` beside the other six, in the same `match` shape with the
same logging.

**Tests.** A row inside the window survives; a row past it is deleted; the
count returned is the number deleted. One test per status that the predicate is
meant to reach, asserting on the *status*, not only on the count — a predicate
that deletes the right number of the wrong rows must fail. And an
`expires_at = now - 24h` exact-boundary case: state which side it falls on and
make the test pin it, in the shape stage 3's claim boundary got.

## Item 2 — `:600-601`'s cookie. Expired on two terminal paths of twenty-three

RFC 096 `:600-601`: *"The callback cookie is expired on every terminal path."*

Measured in `federated_callback`: **23 `return Ok(...)` terminal paths. Two
remove `STATE_COOKIE`** — the upstream-error branch and the state-mismatch
branch. The other 21 return a bare `Redirect` and leave the cookie in the
browser until its own max-age. `BROWSER_BINDING_COOKIE` (`sui_id_fed_browser_binding`)
is removed on **no** path at all, including the successful one.

**This is not a live replay hole, and do not describe it as one.** Stage 3 made
the attempt row the one-time authority: a replayed state cookie now fails at
`claim` with `StoreError::Conflict`, and the nonce is consumed. What is wrong is
narrower and still worth fixing: the RFC requires the expiry, two cookies
scoped to `/auth/federated` outlive the exchange they belong to, and a reader
comparing the code with `:600-601` finds a contradiction.

**What to build.** Every terminal path out of the callback expires both
cookies — `STATE_COOKIE` and `BROWSER_BINDING_COOKIE`. Including the success
path, which currently removes only the state cookie
(`complete_federated_signin`), and including `complete_federated_signin`'s own
`signin_failed` and pending-MFA returns.

**Do it so the next path cannot forget.** Twenty-three `return` sites each
gaining two `.remove(...)` calls is a pattern that decays the first time
someone adds a twenty-fourth. Find the shape that makes it structural — one
helper that every terminal return goes through, a wrapper applied once where
the response leaves the handler, or whatever the handler's own structure
actually supports — and **say what you chose and what it would take to bypass
it**. If the honest answer is that nothing enforces it and a comment is the
best available, say that instead of implying more.

**Tests.** Pick terminal paths of genuinely different shapes — an early refusal
before `fed_state` is unsealed, a mid-exchange failure, the `link_required`
return, the success path — and assert the `Set-Cookie` headers expire both
cookies. The e2e harness already reads `Set-Cookie`
(`extract_set_cookie`, and `tests/e2e/r096_b1_stage5.rs` drives four of these
paths end to end).

## Not in scope

**No command, no audit event, no manifest row.** If you find yourself wanting
one, that is the signal you have crossed into F05 — stop and say so.
`contracts/write-commands.toml` should be untouched, so **G17 is owed
nothing**; confirm that against the index rather than by intention.

**No terminalization.** Do not set `status = 'failed'` anywhere. That is F05,
and it is held. A purge that deletes a row is not a terminal transition.

**No change to the legacy identity-mapping path.** Stage 12 owns its removal.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The purge**, its predicate stated in prose and justified against the
   schema's actual columns, and its `gc.rs` registration.
2. **The cookie expiry on every terminal path**, with the count — if it is not
   23 plus `complete_federated_signin`'s own, say what the real number is and
   where my count went wrong.
3. **Whatever makes item 2 structural**, named, with an honest statement of
   what would bypass it.
4. **Mutation evidence** for both: disable the purge predicate's time bound,
   and remove the expiry from one terminal path, and show which test fails in
   each case. The second one is the interesting one — if removing it from a
   path fails nothing, the tests are checking the helper rather than the paths.
5. **Per-hunk SHA-256** against the tip you named, and full-content hashes for
   any new file.
6. **Gate evidence** from a throwaway clone with its own `target/`, plus
   `cargo test --workspace` on both pinned toolchains. **G13 and G20 are owed**
   — you are adding a store-crate write path, even though it is not a declared
   command, and G20 reads an artefact the test run writes.
7. **Anything you think is wrong with this dispatch.** Two numbers in it are
   mine and checkable: *23 terminal paths* and *six tables in `gc.rs`*. The
   predicate in item 1 is deliberately not specified, so if the schema will not
   support the rule I gestured at, that is a finding and not a blocker — say
   so and propose the rule that the columns do support.
