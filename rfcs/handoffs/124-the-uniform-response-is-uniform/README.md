# The uniform response must be uniform, and must be shown to be

**RFC.** [RFC 124](../../accepted/124-the-uniform-response-is-uniform.md), **Proposed**.
**Author.** High-capability model, requirements-architect role.
**Baseline.** `4ebf0f7` or later.
**Two stages. Stage 2 does not start until stage 1's measurement is reviewed.**

## Why this handoff exists at all

`@nabbisen`, 2026-09-29, on the architect's suggestion that the timing be
"folded in as a measurement rather than an RFC": **"No record? No docs? No
handoff? Proceed carefully."** He was right, and this is the correction: the
measurement gets an RFC, a handoff, a committed artefact and a documentation
outcome, like any other work. A measurement done informally leaves the next
person exactly where this one started — with a claim and nothing behind it.

## Stage 1 — measure, change nothing

**Dispatched 2026-10-01**, on `@nabbisen`'s acceptance.

**Read RFC 124's security review first** (`security-review-2026-10-01.md`). It
found **six** branches, not two, and the work increases monotonically across
them. That changes what this stage measures: not "exists versus not" but **where
on a six-rung ladder a given address sits**. Three rungs disclose more than
existence — a never-activated account, a disabled one, and one with reset tokens
already outstanding.

**So measure the branches that matter, not two.** At minimum: unknown address;
local account that has never been activated (the extra `credentials::get`);
active account under the token cap. The never-activated rung is the sharpest,
because it fingerprints exactly the accounts RFC 115 exists to protect.

### Confirm first, at the baseline

- The claim itself: `sui-id-core/src/account/forgot_password.rs:32` states the
  request "takes roughly the same time in both branches".
- The known-address path inserts a reset token (`:174`), reads SMTP
  configuration, inserts into the outbox, and writes more audit events; the
  unknown-address path returns after one lookup and one event.
- **The production mailer is the persistent outbox** (`runtime/startup.rs:243`),
  so the send is a local encrypt-and-insert, not inline SMTP. **Confirm this**,
  and confirm whether a deployment can configure an inline SMTP mailer instead —
  if it can, the difference is far larger and this RFC's framing changes.

### The measurement

**Method, stated so it can be re-run rather than believed.** Both branches, same
process, enough samples to say something, **distribution not average** — p10,
median, p90, max, as RFC 123's own measurement reported. Name the environment:
machine, build profile, and whether the timing is taken in-process or through
the real HTTP path. In-process is acceptable and RFC 123's precedent shows why —
a large enough gap survives network jitter without careful statistics — but
**say which you measured**, because the honest conclusion depends on it.

**What the numbers must answer, explicitly:**

1. Is the difference distinguishable at all under this method?
2. If it is, **what would an attacker need** — how many samples per address, over
   what network — to classify "this address exists" with confidence? A difference
   that exists but needs ten thousand samples per guess is a different finding
   from one that needs three.
3. If it is **not** distinguishable, say what method *would* distinguish it, so
   the claim's limits are known rather than assumed absent.

**The artefact is a committed file** under this handoff, with the method, the
environment, the raw numbers and the code that produced them — the shape RFC
123's `d4-measurement.md` established. A measurement nobody can re-run is an
opinion with a number attached.

### Do not

Change behaviour. Tune the code to improve the numbers. Or report a conclusion
the numbers do not carry — **"this claim should simply be deleted" is a
legitimate outcome**, and so is "the asymmetry is real but unexploitable, and
here is why".

## Stage 2 — dispatched 2026-10-01, on the measurement

Stage 1 landed (`318e33f`). Its numbers decide what this stage is:
**branch 6 at 234–380 µs against 19–37 µs for the other five** — an order of
magnitude, trivially distinguishable, separating a healthy actionable account
from an unknown address, a **disabled** one, and one **already mid-recovery**.
The five small branches differ by microseconds and are impractical remotely.

**So the work is structural, not work-matching** — RFC 124 D1 as amended. Five
matched costs maintained forever is five places a future edit reopens a gap; one
code path before the response has none.

### The architect's decision, and its cost

**The request path does one thing: it durably records that a recovery was
requested for the submitted address, then responds.** Everything else — the
`UserSource` filter, the credentials lookup, the disabled check, the
outstanding-token throttle, the token mint, the token insert, the SMTP read, the
outbox insert — happens in a worker, after the response.

**This needs a new durable row, and therefore a migration.** I considered
spawning a task instead, and rejected it: a spawned task loses the request if the
process restarts, so a user would be told "request received" and never receive
mail — **the exact failure D6 and D7 exist to end**, reintroduced by the fix for
D1. A side-channel fix that creates a silent correctness loss is not a fix.

**State the cost plainly in the package**: RFC 112 made a binary refuse a database
newer than it understands, so a migration means this release is **not rollback-safe
with the binary alone**. That is the price, it is known, and it is accepted for a
gap that distinguishes a healthy account from a disabled one.

**If you find this infeasible or find a third structure, say so rather than
building around it.** Feasibility is yours; the choice between durability and a
migration is mine and is made.

### Also in this stage

- **D1's amplification bound**: confirm the per-IP five-per-sixty-seconds limit
  bounds unconditional recording, rather than assuming it. If it does not, stop
  and say what bound does.
- **The hand-off must not be conditional on anything address-derived**, or the
  ladder returns in a new place. This is the thing to check hardest.
- **D6 — no screen asserts what may be false.** The title says "Email sent" in
  all three locales above a body saying "if an account exists". It becomes true
  for every caller: what happened is that the request was received.
- **D7 — a route forward.** A user whose address is not registered currently gets
  "check your spam folder" and nothing else. Say, without revealing which case
  the reader is in, that if nothing arrives the address may not be the registered
  one, and what to do then. **This costs no secrecy**: it is equally true for both
  callers.
- **D8** — the three locales' new strings join the existing native-review queue;
  do not start a second one.

### Evidence

- The six branches re-measured after the change, by stage 1's own method and
  artefact format, showing the request path's cost no longer varies by branch.
- A test that the response is byte-identical and the status identical across all
  six.
- A test that a recovery request survives a restart between the response and the
  worker draining it — the property the spawned-task alternative would have lost.
- Mutations: make the recording conditional on the lookup; drop the worker.
- fmt and clippy **through `scripts/ci-gate.sh`**, not by hand — a hand-run
  `cargo fmt` passed while the gate's `cargo +stable fmt` failed on 2026-10-01.


## Stage 2 reviewed 2026-10-01 — accepted, with two required changes and a protocol breach

**The work is correct.** 40 of 40 declared hunks and all four new files match
`d031438..4f58066`; eight gates and 1062 tests pass. The structure is what D1
asked for: the handler's only write is one unconditional `INSERT`, the six
branches all moved into `ForgotPasswordWorker`, and the failure path deletes the
row with its reasoning stated in place — best-effort and unretried, exactly as
the pre-RFC-124 handler's `let _ = request_reset(...)` already was, so no
behaviour regressed and there is no retry loop.

### Breach: it was committed and pushed to the public remote unreviewed

`4f58066` reached `origin/main` before any review, carrying a **schema migration
to version 44** and two new entries in `contracts/write-commands.toml`. Under
RFC 112, a database migrated to 44 makes an older binary refuse to start, so this
is not a change that can be undone by checking out an earlier commit.

Nothing bad happened — the work is sound, no release was tagged, and no
deployment is known. **It is recorded because the protocol exists for the case
where the work is not sound**, and because a migration is the single worst thing
to push unreviewed. The protocol is unchanged: the dev team hands over a working
tree; the architect verifies, commits and pushes.

### Required change 1 — one hunk was not declared

`contracts/write-commands.toml` `@@ -1144 +1144` registers **O05** and **O06**
and appears in no hash list. **Its content is correct** and the gates accept it.
But `contracts/` is G17's registry of every Class-A write, and the per-hunk
hashes are the only mechanism by which an undeclared change there is caught —
which is how this was found. Declare it.

### Required change 2 — the plaintext justification cites a precedent that does not apply

`0044_forgot_password_requests.sql` justifies storing `email` in plaintext as
*"matching `users.email`'s existing precedent"*. **It does not match.**
`users.email` holds the address of someone who registered. This column holds
**any address anyone submitted**, including an attacker's probe list for
addresses that do not exist — and `events.rs:56` records
`PasswordResetRequested { user_id }` with a comment saying the address is
deliberately *not* recorded, *"so an attacker probing the endpoint cannot derive
matched-vs-unmatched from the actor column"*.

So this RFC newly persists, transiently, exactly what the audit event was
designed not to. **The architect's decision: keep the column, correct the
comment, and state the retention.** The rows are transient by design and deleted
after processing; a stalled worker means submitted addresses accumulate, and that
is an operational fact an operator must be able to find — so it goes in `docs/`
under RFC 127's rule, not only in a migration comment. **No pruning mechanism**:
if the worker is down, account recovery is broken and the operator has a larger
problem than this table.

