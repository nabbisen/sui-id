# RFC 096-B1 stage 9 — F02: the first federated MFA continuation, and the purge that goes with it

**Dispatched.** 2026-10-14 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
its stage 9.
**Prior.** Stages 0–5, 6a–6c, 6e, 7 and 8 complete. Stage 8 landed
`d7d5523`, after one fix round.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

**This is the first writer of the two tables stage 8 built**, and the stage
that has to make the legacy federated-MFA branch tell the truth. RFC 094 F02:
*"the Protocol-class existing-link/local-MFA-pending transaction"*
(`094:549`), `class = "P"`, `status = "target-legacy"` in
`contracts/write-commands.toml:805-813`.

## Scope

### 1 — one `db.protocol` transaction, two writes

`Database::protocol` (`registry.rs:960`) is the runner — **not `class_a`**.
Class P has no path to `Audited<T>` by construction, and that absence is the
proof (`registry.rs:948-954`). Inside it, exactly two writes:

- the **guarded attempt completion**, `exchanging -> completed`:
  `federation_login_attempt::complete_within_tx` already exists and is what
  F04 uses (`commands.rs:2324`). A lost race is `StoreError::Conflict`.
- **exactly one** `federation_mfa_pending` row, through a new
  `*_within_tx` repo function.

**Where the function lives is forced, not a preference.** `WriteTx::tx()` is
`pub(crate)` (`registry.rs:105`), so only `sui-id-store`'s own code can reach
the raw transaction — the handler in `sui-id` cannot open a `db.protocol`
block and call a `*_within_tx` inside it. The one existing Protocol command,
**T09** (`commands.rs:1958-1966`), is the shape to copy: a `pub async fn` in
`commands.rs` whose whole body is
`db.protocol(move |write| repos::…::…_within_tx(write.tx(), …))`. Its doc
comment is also the model for what to say — *"no event, no audit row is
possible here by construction"*, with the reason this command is Protocol and
not Class A.

Both or neither. **Today neither happens**: read `handlers/federation.rs`
`:1610-1644` — the `mfa_enabled` branch issues a *generic* pending row through
`sui_id_core::mfa::issue_pending_mfa` and returns, and **never completes the
attempt**. The attempt sits in `exchanging` until the 24-hour purge collects
it. Nothing is exploitable there (a second callback cannot re-claim a
non-`pending` row), but the row's status stops meaning what it says, and F02
is where that is fixed.

### 2 — the rechecks, and precisely what stage 8's schema does *not* cover

RFC 094 `:625-627`: *"F01/F02/F04 recheck that the provider is enabled at the
attempt's exact version and activation generation. F01/F02 additionally
require the same active link and active user."*

Stage 8's new `FOREIGN KEY (user_id, provider_id) REFERENCES federation_link`
gives you **link existence, and nothing else**. It does not check that the
user is not disabled or deleted, that the provider is still enabled, or that
its config version and activation generation still match the attempt's. Do
not let the FK's presence absorb those — say in your package, per recheck,
which mechanism holds it and where.

State each recheck as the expression that runs, not as prose. The model is
`federation_login_attempt::claim` (`repos/federation_login_attempt.rs`), which
already does the provider triple as a single `!=` chain and the browser
binding with `subtle::ConstantTimeEq`.

### 3 — the sealed continuation: say what is actually in it

`:721-722` says the challenge preserves *"`primary_method=Fed`, provider/link
identity, next target, creation/expiry, and one-time consumption."* Look at
stage 8's table and notice that **every one of those is already its own
column** — `primary_method`, the provider triple and `user_id`, `next_path`,
`created_at`/`expires_at`, and `status`.

So the first question is not how to seal the continuation but **what it
contains that the row does not already carry**. The schema requires it to
contain *something*: `continuation_sealed IS NOT NULL` is half of the
pending/terminal `CHECK`. Three answers are defensible and I am not choosing
for you:

1. It carries state that genuinely has no column — say what, and why it
   should not be one.
2. It is a sealed restatement of the row's own identity, and its value is
   that a promotion must *open* it with the row's own values as AAD, so a row
   edited in place out of band no longer promotes. That is a real property;
   if this is the answer, say it in those words, because it makes the column
   a tamper check rather than storage.
3. Something else you argue for.

What is **not** acceptable is a blob whose purpose nobody can state, kept
because the RFC used the word. If you reach that point, say so and it comes
back to me.

Whatever it holds, follow `pkce_verifier_sealed`'s rule (migration `0046`'s
own comment): the **AAD is reconstructed from the row** at open time, never
stored beside the ciphertext. For this row the natural AAD is the pending id,
provider id, config version, activation generation and user id — all five
already columns.

### 4 — the two binding digests

`browser_binding_sha256` and `csrf_binding_sha256`, both `BLOB` 32 and both
`NOT NULL`. For each, state in the package:

- where the raw value comes from — `:564`'s 32-byte CSPRNG is the precedent
  for anything newly minted;
- what goes to the browser and what goes in the row (**the digest, never the
  raw value**);
- that comparison at consumption time is constant-time (`subtle::
  ConstantTimeEq`, as `claim` does). Stage 10 does the comparing; stage 9
  must not make it impossible.

The browser binding should be the same *kind* of value the attempt already
carries. Whether it is the same value or a fresh one is your call — argue it.

### 5 — `expires_at = created_at + 300`

The schema `CHECK` is whole-second epoch equality with `IS NOT NULL` guards on
both sides. Compute both columns from **one** `DateTime<Utc>`, not from two
`now()` calls — two calls can straddle a second boundary and the row is then
refused at insert, which is a 500 at the worst moment. A test with a
fractional-second `created_at` is cheap insurance.

### 6 — the purge, which is this stage's

`:734`: *"Terminal rows ... are purged after 24 hours."* Stage 8's dispatch
recorded it as stage 9's, with the first writer, and stage 6a's lesson is that
an unpurged table is a growth leak from its first writer onward.

Model: `federation_login_attempt::purge_expired(db, now)` — `DELETE ... WHERE
expires_at < ?1` with the cutoff at `now - 24h`. Three things to get right
and to say:

- **Do not filter on `status`.** `:734`'s sentence is about terminal rows,
  but a `WHERE status != 'pending'` purge leaves every *abandoned* challenge
  — a user who closed the tab at the TOTP prompt — in the table forever. The
  attempt table's own purge is by expiry alone for exactly that reason, and
  `expires_at` here is `created_at + 300s`, so a 24-hour cutoff past expiry
  is generous for both kinds of row. If you think the RFC means the narrower
  thing, say so rather than quietly widening it.

- **Where it is called from.** A purge function nobody calls is the growth
  leak with extra steps. Wire it into whatever already calls the attempt
  purge, and name that call site.
- **The ceremony table.** `federation_webauthn_ceremony` cascades from
  `federation_mfa_pending` (stage 8's composite FK, `ON DELETE CASCADE`), so
  purging parents collects ceremonies too. Confirm that with a test rather
  than asserting it, and say whether the ceremony table needs its own purge
  for rows whose parent is somehow absent — it cannot have any, and the test
  should show why.

### 7 — the audit record, which is currently false

The legacy branch writes `auth.login.password_ok_mfa_required`
(`handlers/federation.rs:1637`). For a federated sign-in no password was
presented, and for a user F04 provisioned there **is no password at all** —
that user is created passwordless, can enrol TOTP, and then reaches exactly
this line. The operator guide renders the event as *"Password accepted"*
(`docs/src/guides/operators.md:833`) and the reference as *"Password was
correct"* (`docs/src/reference/audit-events.md:17`). An operator reading the
log is told something that did not happen.

`contracts/audit-coverage-matrix.md:339` does say the event is *"written best
effort by every sign-in path that has a second factor, by design (RFC 102
A6)"*, so the reuse was deliberate once. RFC 096 `:723-724` — *"it never
substitutes `Pwd`"* — and the passwordless case are what overtake it.

**My instruction: F02 does not write that event.** What replaces it is the
part I want you to argue, and these are the two candidates I see:

1. **A federation-specific must-attempt observation**, e.g.
   `auth.federation.mfa_required`, through `emit_must_attempt` — a new
   descriptor, a `contracts/audit-coverage-matrix.md` row, and the matching
   `docs/src/reference/audit-events.md` row that G15's check (D) requires as a
   set equality. Honest, and it keeps a record that exists today.
2. **Nothing at all**, on the grounds that Class P writes no event and the
   pending row is itself the durable trace.

I lean to (1) — removing the only record of "a federated sign-in stopped at
the second factor" makes the log quieter about a security-relevant moment,
and `094:549`'s *"no login-success event before MFA"* forbids a *success*
event, not an observation. But if (2) is right I would rather be shown. If
you take (1), the event name is yours to propose, and the class is **B /
must-attempt**, not atomic: `094:120` A6 — failure to log never prevents the
denial, and the same reasoning makes it never prevent the challenge.

Either way, the matrix and the reference page move together, and the operator
guide's sentence for the *password* event stays as it is — it is true there.

**If you add a descriptor, write it in the shape the gate can see.**
`scripts/rust_registry.py:114` recognizes `MustAttemptDescriptor` statics
since stage 6b, but by the same regex shape as `:108`'s `EventDescriptor`
one: `static NAME: MustAttemptDescriptor = MustAttemptDescriptor {` on one
line, the type unqualified, the event name an inlined literal. A descriptor
split across lines or written through a type alias is invisible to G13, and
the matrix row then has nothing holding it to the source. I checked this
before writing it — stage 6b's review found the opposite state and the fix
landed.

### 8 — not in scope

**F03, F06, `FederatedMfaVerifier`** — stages 10 and 11. Write no consumer.
**The ceremony table's first row** — stage 11.
**The pending-MFA cookie on terminal paths** (S1) and the legacy
`/admin/login/mfa` handler's own federated branch — stage 12 owns the
switch-over. If F02's row cannot be consumed by anything yet, say so plainly
in your package and name what stage closes the loop; do not half-wire a
consumer to make it look finished.

**G20 does not apply to F02.** Its declared set is the
`command ID = "ID"` rows in `commands.rs` (25 today, all Class A, each
required to reach a `ClassATx<'_, ID>` seam). A Class-P command declared that
way would fail the gate. F05, F07 and F08 are precedent: Protocol commands,
not in that macro, not in G20's registries. **That means no gate will ask you
for rollback evidence here** — which is exactly why I am asking for it by
hand (§What to return, item 4).

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The transaction**, with each recheck named as the expression that runs
   and the mechanism that holds it (§2), including the three the FK does not.
2. **The continuation's AAD**, stated, and the sealing/opening sites.
3. **The binding digests**: origin, what the browser gets, what the row gets.
4. **Rollback evidence, with a control.** A failure injected before commit
   leaves no pending row **and** the attempt still `exchanging`; the same test
   runs a second, untouched identity and shows both writes land for it. No
   gate will ask; the property is the point.
5. **The purge**: the function, its call site, and a test that a terminal row
   older than 24 hours goes and a fresh one stays — plus the cascade test for
   the ceremony table.
6. **The audit decision** (§7), argued, with the matrix and reference rows if
   you take (1).
7. **`contracts/write-commands.toml`'s F02 row** flipped from `target-legacy`
   to `implemented`, with real `files` and `test_id`. G17 checks it.
8. **Per-hunk SHA-256** against the tip you named, full-content for new files.
9. **Gate evidence**: the full matrix from a throwaway workspace with its own
   `target/`. **G13 and G15 are the ones this stage can actually move**, if
   you add an event — G13 holds the matrix's class/actor/step_up columns to
   the sealed descriptors, and G15's check (D) is a set equality between the
   matrix and `docs/src/reference/audit-events.md`. Say what each checked,
   not only that it passed.
10. **The count reconciliation** from `sui-id-store --lib` **448**,
    `sui-id --lib` **560**, `sui-id --test e2e` **532**, `sui-id-i18n` **19**,
    `scripts/tests` **349** — measured on stage 8's landing commit, by me.
11. **Anything you think is wrong with this dispatch.** I traced the readers
    of `auth.login.password_ok_mfa_required` before writing §7 rather than
    leaving it to you: two writers on the local password path
    (`sui-id-core/src/authn/session.rs:240`,
    `handlers/admin/auth.rs:355`), one e2e assertion that is a *password*-path
    count (`tests/e2e/r102_stage3.rs:758`), the matrix row and the two doc
    pages. No metric, no admin-UI reader, no federation test. So nothing
    downstream loses a signal when the federated path stops writing it — the
    open part is only which record replaces it. If you find a reader I
    missed, that is the finding.
