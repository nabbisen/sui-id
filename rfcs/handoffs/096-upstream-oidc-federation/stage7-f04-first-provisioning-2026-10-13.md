# RFC 096-B1 stage 7 — F04: first federated provisioning, in one transaction

**Dispatched.** 2026-10-13 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md).
This is its stage 7, the first dispatch after `0.80.0`.
**Prior.** Stages 0–5 and 6a–6c, 6e complete. `0.80.0` shipped from `7aecd98`.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

**This is federation's first use of the Class-A runner**, and the plan says it
needs the heaviest review of what remains. Expect a long round.

## What exists today, measured

`federated_callback`'s `ProvisionOnFirstLogin` arm provisions inline, in
**three separate writes with no transaction between them**
(`handlers/federation.rs:1344-1400`):

1. `derive_username(&id_claims)` then `resolve_shadow_username(&app.db, …)` —
   a read, outside anything.
2. `users::upsert_ldap_shadow(&app.db, shadow, now)` — its own `with_conn`.
3. `federation_link::upsert(&app.db, …)` — its own `with_conn`.
4. `audit::append(…)` — best-effort, after the fact.

A crash or a concurrent callback between any two of those leaves a user with
no link, a link with no audit row, or two callers racing the same username.
**That is what F04 collapses into one `WriteTx<AtomicAudit>` transaction.**

## Scope — RFC 096 `:706-711`, and the manifest row

> `provision_on_first_login` requires a present email and
> `email_verified=true`. If any local user already has that normalized email,
> deny as a takeover collision; never auto-link. Otherwise derive a bounded
> local username under RFC 004 P7 and execute amended RFC 094 F04: **attempt
> completion, passwordless non-admin user, link, `[Fed]` session/cap, and
> `auth.federation.provisioned` commit in one Class-A transaction.** An absent
> or unverified email denies; M4 does not invent a held-account state.

`contracts/write-commands.toml`'s F04 row adds the guards:

> require enabled exact-version/generation provider; guard attempt
> `exchanging -> completed`; **recheck** absent link + verified unique email +
> username uniqueness; create passwordless non-admin user; create exactly one
> provider/sub link; insert `[Fed]` session; enforce cap; append sole intent
> event

### 1. Everything in one transaction, and the rechecks are the point

The guards above are not a repeat of what the handler already checked — they
are the same questions asked **inside** the transaction, because the answers
outside it are stale by the time the write lands. Absent link, unique email,
unique username: each was checked before, each is checked again here, and a
disagreement is a lost race, not an error in the caller.

`sign_in_federated` (`commands.rs:2209`) is the one existing Class-A command in
this area and is the shape to follow: `db.class_a(context, |tx| …)` with every
write on `tx.tx()` and the event returned from the closure.

### 2. Do not reuse `upsert_ldap_shadow`

The legacy path creates the user with `users::upsert_ldap_shadow`
(`repos/users.rs:922`), a directory-shadow **upsert** keyed on
`external_stable_id`. F04 must **create**:

- The manifest says *"create passwordless non-admin user"* and *"create
  exactly one provider/sub link"*. An upsert silently accepts the case where
  the row already exists — which is precisely the race the recheck exists to
  catch, hidden instead of refused.
- Borrowing LDAP's writer also stamps a federated user with directory-shadow
  semantics it does not have.

There is an in-transaction variant (`upsert_shadow` on the caller's
transaction, `:931` onward) — **read it before deciding**. If it genuinely is
the right primitive and the upsert semantics can be made a create, say so with
the argument; if not, say what you built instead and why. **Do not silently
reuse it**, and do not silently fork it either.

### 3. The absent-email denial, which is also a documentation fix

`:710-711` — *"An absent or unverified email denies"*. Today the legacy P3
check is `email.is_some() && !email_verified`, so an **absent** email falls
through and provisions. Stage 5's resolution already models the correct
answer (`DeniedUnverified` covers both absent and unverified) and stage 6c
deliberately left the legacy behaviour alone, saying so.

**This stage is where it changes.** And when it does,
`docs/src/guides/operators.md:1594-1596` becomes true for the first time —
it says `provision_on_first_login` *"requires `email_verified = true` from the
upstream; otherwise the sign-in is rejected"*, which is currently false for the
absent case. **Confirm in your package that the sentence has become true**, and
if you find it needs rewording rather than merely becoming true, reword it.

This is a behaviour change for an operator whose upstream omits `email`:
such a user is provisioned today and denied after this stage. **Say so in the
package** so the next release's changelog carries it.

### 4. The event, which does not exist yet

`auth.federation.provisioned` appears in **no** descriptor, **no** matrix row
and **no** reader reference — grepped. This stage creates all three:

- A sealed `EventDescriptor` with `class: AuditClass::Atomic`, declared
  through `declare_write_command!` as F04's own event, with the attributes the
  manifest names: *"new-user sealed-federated actor/target, internal
  provider/link IDs, fixed mode"*. **Internal ids only** — `:714-715`, no raw
  email, no upstream `sub`.
- A row in `contracts/audit-coverage-matrix.md` claiming **A**, which G13's
  column check then holds to the descriptor's own `actor`.
- A row in `docs/src/reference/audit-events.md` — **G15 check D**, the gate
  that caught stage 6c for exactly this omission. Do not make me find it.

Mind the declaration shape: `rust_registry.py` wants an unqualified type on
one line with an inlined literal name, and stage 6c added a guard that fails
if a declaration drifts out of that shape. The guard should stay silent; if it
does not, you have written the descriptor in a form the gate cannot see.

### 5. The manifest row

F04 becomes `status = "implemented"` and **`sealed = true`** — and
`check-write-commands.py` requires `sealed = true ⟹ class = "A"`, which F04
already is. `files`, `mutation_surface` and `test_id` get the same treatment
F05's row got in stage 6c: what the code does, where it is, and tests that
exist.

## Carried in from earlier reviews

Three items were assigned to "whichever stage carries F04". This is it.

1. **`L04`'s audit event carries the upstream `sub`** —
   `commands.rs:2144-2159` declares it as an `AttributeSpec`, *"the upstream
   subject, truncated to 255 bytes"*, and it lands in the note on every
   successful federated sign-in. `:714-715` says audit uses internal ids. Drop
   the attribute, update the two literal-note assertions at
   `tests/e2e/r102_stage4.rs:498` and `:584`, and make sure F04's own new event
   does not repeat the mistake. RFC 102's text does not name this attribute —
   checked — so there is nothing to amend, only a descriptor to change.
2. **The duplicate `federation_link::find_by_sub`.** Every callback runs it
   twice: once in `identity_resolution.rs` and once in the legacy path. The
   resolution is still not authoritative, so **collapse only if you can do it
   without making it so** — if the honest answer is that it waits for stage 12,
   say that instead of forcing it.
3. **`operators.md:1594-1596`**, §3 above.

## Not in scope

**The legacy path's removal.** Stage 12. F04 takes over the provisioning
branch; the rest of the legacy mapping stays and still decides the other
outcomes.

**F01, F02, F03, F06.** 6d and stages 8–11.

**The RFC 094 amendment.** Sequenced into the gap after stage 11; stage 6d
waits on it. Nothing here touches it.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The transaction**, with every write named and the proof that each is on
   `tx.tx()` rather than its own connection.
2. **The three rechecks**, each with a test that loses the race deliberately —
   a link, a user with the email, and a user with the username, each created
   between the handler's check and the transaction, each refused.
3. **The user-creation decision** (§2), with the primitive you chose and the
   one you rejected.
4. **The absent-email denial**, its test, and the operator-guide sentence
   confirmed true.
5. **The event**: descriptor, matrix row, reader-reference row, and the
   attribute set with the reasoning that none of it is claim data.
6. **Rollback evidence.** Inject a failure at the last write and show that the
   user, the link, the session and the event are all absent afterwards —
   **this is the property the Class-A runner exists for** and the one the
   stage is judged on. RFC 094 M2a's criterion 2 is the model: *"injected
   append failures roll back mutation for every converted row"*.
7. **Exactly-once evidence.** One successful provisioning writes precisely one
   audit record — not zero, not two. M2a's criterion 3.
8. **The manifest row**, and **G17** green with `sealed = true`.
9. **Mutation evidence**: move one write out of the transaction; break the
   link recheck; make the event's attributes carry the `sub`. Each should fail
   exactly one thing.
10. **Per-hunk SHA-256** against the tip you named, full-content hashes for new
    files.
11. **Gate evidence**: the full matrix from a throwaway clone with its own
    `target/` — note that `target/` was cleared at the release boundary, so the
    first build is cold for everyone — plus `cargo test --workspace` on both
    pinned toolchains. **G13, G15 and G17 are all owed.**
12. **The count reconciliation** from `sui-id --lib` **560**, e2e **528**,
    `sui-id-store --lib` **393**, `sui-id-i18n` **19**, `scripts/tests` **349**.
13. **Anything you think is wrong with this dispatch.** The claim most worth
    checking is §1's — that the legacy path's three writes are genuinely
    untransacted. I read `:1344-1400` and saw three separate `with_conn` calls;
    if one of them is already inside something I missed, the whole framing of
    this stage changes and I would rather hear it now.
