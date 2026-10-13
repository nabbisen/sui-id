# RFC 096-B1 stage 9, fix round — a binding the browser can still present

**Dispatched.** 2026-10-14 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Prior dispatch.** [`stage9-f02-first-mfa-continuation-and-the-purge-2026-10-14.md`](stage9-f02-first-mfa-continuation-and-the-purge-2026-10-14.md)
**Review result.** `.git-exclude/reviewed/rfc-096-b1-stage9-f02-first-mfa-continuation-and-the-purge-2026-10-14.md`
**Baseline.** Your returned working tree on
`b10384fdf1fdcb0e6bc330e8389fdf783ef9124c`. Keep it; do not rebuild from the
baseline commit.

**The transaction, the purge, the audit replacement and the `Database::
protocol` fix are all accepted as returned.** §0 is the find of the stage and
I verified its blast radius independently: three runners in the crate, two
pre-existing callers, both single-write. Nothing there changes.

Six changes. **R1 is the only one that is a defect**; the rest are a missing
sentence, two missing assertions, a missing hash table, and two stale prose
sites that are my omission to have caused.

---

## R1 — `browser_binding_sha256` commits to a cookie this very response deletes

**Measured, not argued.** I printed every `Set-Cookie` on the callback
response for
`federation_fail_closed::fed_user_with_mfa_gets_the_mfa_step_not_a_session`
(probe added, run, reverted byte-exact):

```
sui_id_fed_mfa_pending=…;   HttpOnly; SameSite=Lax; Path=/;                Max-Age=300
sui_id_fed_mfa_csrf=…;      HttpOnly; SameSite=Lax; Path=/;                Max-Age=300
sui_id_fed_browser_binding=;          Path=/auth/federated; Max-Age=0; Expires=…
```

The row stores a digest of `sui_id_fed_browser_binding`'s raw value, and the
same response revokes it. `terminal()` (`handlers/federation.rs:194-198`)
removes that cookie on **every** `Ok` path by construction — which is the
property stage 6a deliberately built, and it is right; this branch is simply
one of the paths it covers. Independently of that, the cookie is set
`Path=/auth/federated` (`:446`), so `/admin/login/mfa` would never be sent it
even if it survived.

Your §3 also says the cookie is *"never consumed"*. It is: `:645-648` reads it
to build `presented_browser_binding` for `claim`.

So as returned, `browser_binding_sha256` is a `NOT NULL` 32-byte commitment to
a secret that is revoked, out of scope for the reader, and never re-issued.
My own dispatch's §4 said *"stage 10 does the comparing; stage 9 must not make
it impossible"* — this makes it impossible.

**Do this.** Mint the continuation's **own** browser-binding value —
`sui_id_core::tokens::random_token(32)`, the same `:564` precedent you already
used for the CSRF value — store its digest in `browser_binding_sha256`, and
set the raw value in a third cookie beside the other two: `HttpOnly`,
`SameSite=Lax`, `Path=/`, `Max-Age=300`, `Secure` under `cookie_secure`.

**Do not** instead widen `BROWSER_BINDING_COOKIE` to `Path=/` or re-issue it
past the callback. That extends the lifetime and the path of an OAuth-callback
secret across the whole site to save minting 32 bytes, and it would undo
`:600-601`'s "expired on every terminal path" for the one path where a user is
still mid-authentication. If you see a third answer that a later request can
actually satisfy, argue it instead — the requirement is only that whatever the
column commits to is presentable.

**Then test the response, not just the row.** Nothing in the suite looks at
this callback's cookie set, which is exactly why a column committing to a
deleted cookie passed every gate and both of us. Assert, on the real router:
the two (now three) continuation cookies present and `Path=/`, and
`sui_id_fed_state`/`sui_id_fed_browser_binding` expired. Say in your package
which assertion would have caught R1.

## R2 — state how each binding reaches the consumer

Your §3 answers what is stored and what the browser gets. The open half is the
*channel*, and it decides whether `browser_binding_sha256` and
`csrf_binding_sha256` are two mechanisms or one written twice.

Two `SameSite=Lax` cookies compared the same way are the same check performed
twice. The split that earns two columns is that the browser binding arrives
**automatically** — proving the same user agent — while the CSRF binding
arrives **in the request body**, as a hidden field `/admin/login/mfa` renders
server-side out of the `HttpOnly` cookie, so a cross-site submission fails
even where the cookie would ride along.

No code need change if you have already decided this. What is missing is the
sentence in `repos/federation_mfa_pending.rs`'s module doc, so stage 10
inherits two different checks rather than discovering it owns the question.
If you think one binding suffices and the RFC's two columns collapse to one,
say that instead — but say it; do not leave it to the stage that has to
compare them.

## R3 — read the three defaulted columns back

`insert_within_tx` omits `status`, `failure_count` and `primary_method` and
leans on migration 0047's `DEFAULT`s. **Your §2's whole argument for an empty
sealed plaintext is that `:721-722`'s items "are already their own column"** —
and no test in this package reads any of those three back. Ten new tests, and
the premise the design rests on is the one thing unmeasured.

One assertion on a row written by `issue_federated_mfa_pending`: `status =
'pending'`, `failure_count = 0`, `primary_method = 'fed'`. That is what fails
if a later table rebuild drops a `DEFAULT` while every other test goes on
passing.

## R4 — the 24-hour boundary

25-hours-stale and fresh bracket widely; neither pins the boundary. The
sibling has exactly the test this one wants —
`federation_login_attempt/tests.rs::the_24_hour_boundary_is_strict`, *"the
boundary, in the shape stage 3's claim boundary got: exactly 24 hours"* — and
your purge is a copy of that purge's shape.

It matters marginally more here. The predicate compares stored `TEXT`
lexicographically, and chrono's `%.f` prints nothing at zero nanoseconds, so
`"…12:00:00+00:00"` sorts *before* `"…12:00:00.000001+00:00"` (`+` is 0x2B,
`.` is 0x2E). Sub-second, harmless at a 24-hour cutoff — and better pinned by
a test than re-derived by whoever next reads the `WHERE`.

## R5 — declare the per-hunk SHA-256s

Dispatch item 8, absent: §8's number went to "not in scope" and the table
never appeared. Your `registry.rs` `md5sum` stood in for it — I recomputed it
here and it matches, which is how I know the tree is the one you measured —
but it covers one file of fifteen.

`python3.14 scripts/hunk-hashes.py --baseline <the tip you name>`, full-content
for the two new files. My own measurement against `b10384f` is §6 of the review
result; compare yours against it for the files R1–R4 do not touch, and say if
any differ.

## R6 — two prose sites this change falsifies, and they are mine to have missed

**`contracts/audit-coverage-matrix.md:340`** describes
`auth.login.password_ok_mfa_required` as *"Written best effort by **every**
sign-in path that has a second factor, by design (RFC 102 A6)"*. That is now
false — the federated path is a sign-in path with a second factor that
deliberately does not write it. Amend the scope sentence, and point it at
`auth.federation.mfa_required` for the federated case.

**`docs/src/guides/operators.md:1137-1140`** closes the MFA section with
*"The audit log records every relevant event: … `auth.login.password_ok_mfa_required`."*
An operator building a "stopped at the second factor" query from that list now
silently misses every federated user. Add `auth.federation.mfa_required`.

`operators.md:833` and `audit-events.md:17` stay as they are — with the
federated path gone, *"Password accepted"* and *"Password was correct"* are
true again, which was the point of the change.

**My dispatch caused this.** §7 told you *"the operator guide's sentence for
the password event stays as it is."* That was right about `:833`, the sentence
I had actually read, and wrong as a general instruction: I traced the event's
*readers* and not its *descriptions*, and there are two more of the latter.

---

## What to return

The same working tree, plus a package under `.git-exclude/review-requests/`
with:

1. **R1's new binding**, the cookie it is set in, and the response-level test —
   including which assertion would have caught the returned shape.
2. **R2's sentence**, or the argument that one binding suffices.
3. **R3's and R4's assertions**, each shown able to fail.
4. **R5's hash table**, against the tip you name.
5. **R6's two prose edits.**
6. **The gate matrix again**, from a throwaway workspace with its own
   `target/`. R6 moves **G13**'s and **G15**'s inputs and R1 moves the e2e
   lane; say what each checked.
7. **The count reconciliation** from `sui-id-store --lib` **458**, `sui-id
   --lib` **560**, `sui-id --test e2e` **532**, `sui-id-i18n` **19**,
   `scripts/tests` **349** — measured by me on your returned tree, not quoted
   from your §10.
8. **Anything you think is wrong with this round.** Two of the six are my
   errors; I would rather have a third found than shipped.

## Not in scope

**The two positional-argument signatures** (`federation_login_attempt::insert`,
`issue_federated_mfa_pending`) — transposable same-typed secrets, real, but
F02 copies the sibling's established shape and fixing one of two would leave
the pair inconsistent. It has a scheduling item of its own.

**F03, F06, `FederatedMfaVerifier`, the ceremony table's first row,
`/admin/login/mfa`'s switch-over** — stages 10, 11, 12, unchanged.
