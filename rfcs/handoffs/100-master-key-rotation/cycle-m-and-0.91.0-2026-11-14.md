# Cycle M and `0.91.0` — the key file reaches the disk, and M4-B closes

> ## Authorized 2026-11-14. `@nabbisen`: 1. *"Accepted."* 2. *"Accepted."* 3. *"Confirmed."*
> 1. **Scope**: §3 as proposed. RFC 100's build is planned for `0.92.0`.
> 2. **Kani**: the harnesses are removed, and the guard is added. **S3 dispatched** —
>    [`../146-totp-step-selection-is-proved/0.91.0-s3-retire-the-kani-harnesses-2026-11-14.md`](../146-totp-step-selection-is-proved/0.91.0-s3-retire-the-kani-harnesses-2026-11-14.md).
> 3. **Stage 14's date** is the owner's. No date is set yet, so the tag waits for it.
>
> **The question before S3 landed: settled 2026-11-14** (`@nabbisen`: *"Agreed.
> Authorized."*). Point 4 of the approved position on formal verification says
> *a proof that nobody runs is removed, not kept, through an RFC*, and named
> these harnesses as its example. Point 9 makes their disposition *a separate,
> small decision*. RFC 086 says a pilot is adopted only by a follow-up RFC
> (`rfcs/done/086-formal-model-checking-pilot.md:92, :109`), and none was
> written, so the harnesses were never adopted. Point 9 governs, and no RFC is
> written. Point 4's example sentence is reworded to say so, and its rule is
> unchanged. **S3 landed the same day**, reviewed, with one architect fold-in —
> `.git-exclude/reviewed/0.91.0-s3-retire-the-kani-harnesses-2026-11-14.md`.

> ## D1 approved, and D2 done, 2026-11-14 (`@nabbisen`: 1. *"Approved."* 2. *"Accepted."*)
> The harness meets RFC 100's last implementation prerequisite, and RFC 100 is
> amended (non-material). D2 rewrote `implementation.md` and updated
> `crash-matrix.md` and `verification.md`. RFC 100's build is planned for `0.92.0`.
>
> ## D1 written 2026-11-14 —
> [`crash-injection-harness-design-2026-11-14.md`](crash-injection-harness-design-2026-11-14.md).
> Its prototype found that **K07's commit is not durable when it returns** (WAL
> with `synchronous=NORMAL`), which stage 1's premise had wrong. **Stage 1 gains
> part A2**, dispatched the same day —
> [`0.91.0-stage-1-addendum-the-reseal-is-not-durable-on-return-2026-11-14.md`](0.91.0-stage-1-addendum-the-reseal-is-not-durable-on-return-2026-11-14.md).

**Proposed.** 2026-11-14, by the architect, on `@nabbisen`'s *"Scope 0.91
carefully."*, after `0.90.0` was published (`44d442d`).
**Whose turn.** Yours, for the three decisions in §6. Until then, the architect
works the design track (§3, D1 and D2), which needs no decision, and the dev
team works the two stages already dispatched.
**Measured on.** `3fd2281`. Every `file:line` below is from that tree.

## 1 — what is actually next, measured against the programme

Four milestones remain before the soak, and two of them are long:

| Milestone | What it waits on | Measured |
|---|---|---|
| **M4-B** (RFC 096-B1, 096-B2) | 096-B1's *"representative live-integration evidence"* (RFC 096 `:45`): stage 14, the live run | 096-B2 accepted 2026-11-12. Stage 14 is **deferred to this cycle**, and `0.91.0`'s tag waits for its record (ROADMAP, cycle L's row) |
| **M2c** (RFC 100) | its build, which waits on **the crash-injection harness approved** (RFC 100 `:10-15`) | Accepted 2026-11-14. No harness design exists yet |
| **M5** (RFC 097) | its design prerequisite: 096-A **and** 096-B Implemented (RFC 097 `:5`) | Proposed. Cannot start before M4-B closes |
| **M6** (RFC 099) | RFCs 097 and 098 Implemented, **and RFC 100 Implemented before M6 entry** (RFC 099 `:6-7`) | Proposed |

So the critical path to the soak runs through **two long poles: RFC 100's
build and RFC 097's threat model.** RFC 097 cannot start until M4-B closes,
and M4-B cannot close until stage 14 runs. RFC 100's build cannot start until
its harness is approved.

**This cycle's job is to unblock both poles**: close M4-B, so RFC 097 can
start, and get the RFC 100 harness designed and approved, so its build can
start in `0.92.0`.

## 2 — why `0.91.0` does not carry RFC 100's build

1. **It may not.** RFC 000: a build waits for its implementation
   prerequisites, and the harness approval is one. Nothing beyond stage 1 was
   dispatched, and nothing beyond it can be yet.
2. **It would hold stage 14 hostage.** Stage 14's slot is `0.91.0`'s prepared
   commit. RFC 100's handoff plans six stages (`implementation.md`). 096-B1
   planned nine and took fifteen. Putting the build here would postpone the
   live run, and with it M4-B and RFC 097, by the whole of that build.
   "Controlled properly in the schedule" would then mean "deferred again".
3. **Stage 1 is worth shipping on its own.** Without it, rotating with
   `SUI_ID_MASTER_KEY` set reseals the database under a new key, while every
   later start keeps using the old one from the environment (`keyring.rs:31-32`,
   `cli.rs:592`). And a power loss right after the reseal can leave the new
   key only in the page cache (RFC 100's prefix 1). Both reach operators who
   rotate today.

## 3 — the proposed scope

### Release content

| # | Work | State | Size |
|---|---|---|---|
| **S1** | **Two rotation fixes** (RFC 100 refresh, item 4): `sync_all` and a directory fsync for the staged key and after both renames; rotation refuses an environment-supplied key, before opening the database | **Landed** 2026-11-14, with part A2 — [`0.91.0-stage-1-two-rotation-fixes-2026-11-14.md`](0.91.0-stage-1-two-rotation-fixes-2026-11-14.md) | small |
| **S2** | **Write primitives with no direct test**: delete `users::set_last_login` (no caller), gate `signing_keys::insert_with_plaintext` to tests, and, for ten primitives with no direct test, a boundary and a mutation test for each WHERE clause | **Landed** 2026-11-14 — [`../094-transactional-audit/primitive-coverage-after-0.90.0-2026-11-14.md`](../094-transactional-audit/primitive-coverage-after-0.90.0-2026-11-14.md) | small to medium |
| **S3** | **The Kani harnesses, and the guard the tests lack** — see §5. On your decision 2 | **Landed** 2026-11-14 | small |
| **S4** | **One false comment**: `keyring.rs:36-41` says the variable is scrubbed, then that it is deliberately left set. Nothing removes it. sui-id spawns no child process outside tests, so only the comment is wrong | **Landed** 2026-11-14, folded into S1's review | trivial |
| **P** | **`0.91.0`'s preparation**: version, CHANGELOG, the full local matrix. Dispatched to the dev team, as the mechanical half of every release is | **Dispatched** 2026-11-15 — [`release-0.91.0-preparation-2026-11-15.md`](release-0.91.0-preparation-2026-11-15.md) | small |
| **S14** | **096-B1 stage 14, the live run against Google**, on P's commit — [runbook](../096-upstream-oidc-federation/096-b1-stage14-live-run-runbook-2026-11-12.md) | **deferred here**; preparation listed in the runbook | your time |

Then the tag. If stage 14 finds a defect, it is fixed before the tag and the
gates run again. That is what the run is for.

**For an operator, `0.91.0` is**:
- a rotation whose new key is on disk before it is relied on;
- a rotation that refuses, instead of silently breaking, when the key comes
  from the environment;
- the first release whose federation path has been run against a real
  upstream.

**No migration and no configuration change** are planned. **Why a minor
version, not `0.90.1`**: S2 removes one public function from
`sui-id-store` and gates another behind a feature, which breaks a caller of
the published crate.

### Design track (no release content)

| # | Work | Why now |
|---|---|---|
| **D1** | **The RFC 100 crash-injection harness**, written for your approval (§4) | It is M2c's last prerequisite. Approved in this cycle, the build starts in `0.92.0` |
| **D2** | **Bring the RFC 100 handoff up to the refreshed design.** `implementation.md` and `crash-matrix.md` predate the refresh: they still build `K04`, name `admin.master_key.database_resealed` and `activated`, and have no stage for the startup key check or for refusing to generate a key over a sealed database (RFC 100 `:103-139`) | A dispatch built from them would build the design the refresh replaced |

### After the tag

- **096-B1's completion assessment**, with stage 14's record, for your
  acceptance. With 096-B2 already accepted, that **closes M4-B**.
- Then **RFC 097 can start**: its design prerequisite is met.

## 4 — the harness: one gap to close before it is approved

RFC 100's test plan injects **short writes and process termination**
(`:298-312`, and the crash matrix's *"Both failure modes, independently"*).
**Neither one loses unsynced data.** A killed process leaves its writes in
the page cache, and the kernel still writes them out. A kill-based test
therefore passes **with or without** every fsync the design requires.

That matters, because **the failures RFC 100's refresh measured are power
losses**: prefix 1 (the key only in the page cache when K07's commit is
durable) and prefix 3 (a rename not yet persisted). Stage 1's own dispatch
says it plainly: *"Durability can't be observed from a test"* (`:59`). With
the RFC's two modes alone, M2c could close without one test that fails when
an fsync is missing.

**D1 will propose a third failure mode: loss of everything not yet synced.**
The design already requires a platform adapter, with exactly durable create,
atomic replacement, no-replace publication, and file and directory fsync
(`implementation.md`, stage 1). Its test implementation records each
operation, and what each fsync made durable. It can then build, on a real
temporary directory, every state that a power loss at operation *k* could
leave, and run recovery against each one. This is the approach of published
crash-consistency checkers (ALICE, CrashMonkey). It needs no privilege and no
special filesystem, so it runs in CI.

This adds evidence without changing the design or its invariants (M1–M6). If
you count a wider test plan as an amendment to an accepted RFC, it goes
through RFC 000's amendment path instead. D1 will say which it needs.

## 5 — the Kani harnesses (S3)

The approved position on formal verification leaves this open as its item 9
(`verification/README.md:99-101`): *wire them up or remove them*. Measured:

- **Nothing runs them.** No workflow, script or gate names Kani, and Kani is
  not installed on this machine.
- **They probably do not compile under Kani.** Each calls `kani::any()` for
  `Role` and `Action` (`authz.rs:205-206`), which needs a `kani::Arbitrary`
  impl. Neither type has one, derived or written (`authz.rs:36`; `Role` at
  `sui-id-store/src/models.rs:18`). Without Kani installed, this is read from
  the source, not run.
- **The properties are already proved by tests that run.** `authz/tests.rs`
  checks P1–P5 for every role and every action, in every CI run.
- **The one thing those tests miss is the thing Kani would have caught.**
  `all_actions()` (`tests.rs:27`) is a hand-kept list. Add an `Action`
  variant, forget the list, and P1–P5 all still pass without checking it.

**Recommended: remove the harnesses, and close the gap they were covering.**
Add one exhaustive `match` with no wildcard arm over `Action`, and one over
`Role`, beside each list. A new variant then fails to compile until someone
looks at the list. The alternative, wiring Kani in, would mean first making
the harnesses compile, then adding a toolchain and a CI job to prove what the
tests already prove. RFC 086 is in `done/` and is not edited; the record of
this change goes in `verification/README.md`.

## 6 — your decisions

1. **The scope**: §3, S1–S4, P and stage 14, with the design track D1–D2.
   RFC 100's build is in `0.92.0`, after the harness approval. Recommended.
2. **The Kani harnesses**: remove them, and add the exhaustiveness guard
   (§5). Recommended. The alternative is to make them compile and wire them
   into a scheduled job.
3. **Stage 14's date** is yours, not the architect's. The run needs the Google
   OAuth client, and, if the architect is to drive the admin steps, the Claude
   in Chrome extension connected. **When you can, say a date, or say "ready".**
   The tag waits for the run, unless you record a further deferral.

## 7 — deliberately not in `0.91.0`

| Item | Why not now |
|---|---|
| RFC 100's build, including the startup key check | §2. `0.92.0`, after D1's approval |
| Wiring `http/federation_cache.rs` into sign-in | Every sign-in fetches discovery fresh. `startup.rs:633-638` says so, and that is safe, only slower. Wiring the cache changes the trust path, so it belongs to 096-C, with the hostile-provider matrix |
| 096-C (hostile-provider matrix, live canary, closure review) | After M4-B. Not a prerequisite of M4-B's closure (RFC 096 `:45`) |
| RFC 097 (threat model) | Its design prerequisite is M4-B closed. It starts after this tag |
| RFC 145 (passkey alone) | Needs your feature-freeze exception (RFC 145 `:8`) |
| RFC 146 (TOTP proof) | Waits for M7, by your ruling |
| A test that signs a real passkey assertion | Needs a new dev-dependency, which is your decision. Not raised this cycle |
| Restricting provisioning to an email domain | A feature; frozen |
