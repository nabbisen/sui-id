# Cycle L and `0.90.0` — federation administration (RFC 096-B2)

> ## Authorized 2026-11-11. `@nabbisen`: *"All your recommendations and directions are accepted."*
> 1. **Scope**: `0.90.0` = RFC 096-B2, with RFC 100's acceptance as a design track.
> 2. **Upgrade**: RFC 096's design stands. Providers are disabled on upgrade until
>    re-enabled, with the four protections in §4 as acceptance criteria.
> 3. **RFC 100's reviewer**: §5 item 3 carried no recommendation, so it is
>    **still open**. It comes back to you when the refreshed RFC is ready.
> 4. **ROADMAP relabel**: done. L is 096-B2, and the store-boundary and
>    embeddable rows are unscheduled.
>
> ## Stage 0 done 2026-11-11: the stage plan, with four design amendments for you —
> [`096-b2-stage-plan-2026-11-11.md`](096-b2-stage-plan-2026-11-11.md).
>
> ## Released and published 2026-11-14
> Tag `0.90.0` at `44d442d`, annotated and signed. Level B `43539676127` (28 jobs,
> 0 skipped) and `Security audit` `43539678886`, both on that commit. Publish run
> `43545234825`: six crates, and the GitHub Release with three assets.
> **096-B1 stage 14 is not in it**: deferred to `0.91.0`'s cycle (`@nabbisen`:
> *"Authorized. Defer it. However, it should be controlled properly in the
> schedule."*), and controlled as ROADMAP cycle L's row says —
> [`096-b1-stage14-live-run-runbook-2026-11-12.md`](096-b1-stage14-live-run-runbook-2026-11-12.md).

**Proposed.** 2026-11-11, by the architect, after `0.89.0` (cycle K) was published.
**Why now.** Scheduling is the architect's to propose and yours to authorize.
This proposes a scope. It does not change any RFC.

## 1 — what is actually next, measured against the programme

The ROADMAP's forward table names **the store-boundary RFC** for `v0.90.0`.
**That RFC is unwritten, and scheduling it is a feature-freeze exception
that is yours** (ROADMAP, *What is actually being decided*, decision 1). The
same is true of RFC 144 (Proposed) and RFC 145 (Accepted, needing its
exception).

**The remediation programme comes first under the freeze**, and M2b's closure
in `0.88.0` unblocked it. The *Execution order* table's item 7 is now open:

| Item | State, measured 2026-11-11 | Buildable now? |
|---|---|---|
| **RFC 096-B2**, M4-B's second half | RFC 096 **Accepted**. Its one prerequisite, RFC 094 M2b, **closed 2026-11-08** | **Yes** |
| **RFC 100**, M2c (master-key rotation recovery) | **Proposed**, with two open questions and a design prerequisite needing an **independent** review (§S1a: you, or an external architect you engage). Written 2026-07-28, before M2a's seam existed | **No**: it needs acceptance first |
| RFC 097 (M5), RFC 099 (M6), M7 | Proposed; M6 requires RFC 100 **Implemented** | No |

**So the only programme work that can be built in `0.90.0` is 096-B2**, and
M6, the step before the soak, waits on RFC 100's acceptance. That makes the
cycle two tracks.

## 2 — what 096-B2 is, and why it matters to an operator today

RFC 096 §096-B2: *"Provider and link commands … on the RFC 094 Class-A
transaction seam: provider enable/disable, link create/delete, and
configuration generation increments."* In the contract
(`contracts/write-commands.toml`), that is five rows with no implementation:

| Row | Command | Today |
|---|---|---|
| C17 | Provider enable/disable, consuming a preflight capability | reserved; `set_enabled` is test-only |
| C18 | Provider delete | reserved; `delete` is test-only |
| C19 | Administrative link create | reserved |
| C20 | Link delete | reserved |
| C23 | Replace a provider's trust policy from configuration: version and generation bump, disable, clear evidence, invalidate old flows | contract row only (`files = []`) |

**What an operator meets today, measured:**
- **A provider is trusted from configuration alone.** `enabled: cfg.enabled`
  (`crates/sui-id/src/runtime/startup.rs:379`) is set with no preflight of its
  discovery and keys. RFC 096's design is that enabling requires a fresh,
  validated preflight, recorded in the same audited transaction
  (`architecture.md:29-49`).
- **Changing a trust field in configuration does not change what is trusted.**
  If an operator edits a provider's `issuer`, startup only warns, by design of
  RFC 134 step 7b (`startup.rs:401-414`). The callback keeps validating against
  the **stored** issuer (`handlers/federation.rs:495, 908, 1211`). An operator
  moving away from a tenant would believe trust had moved when it had not.
  RFC 096 already requires the opposite (`:379-382`): the change must be
  written, and must disable the provider until it is re-enabled.
- **There is no way to turn a provider off.** The code says so itself
  (`startup.rs:410-414`, *"nothing in this codebase actually exposes a way to
  toggle it today"*), while a comment earlier in the same file (`:319`) claims
  the admin UI can. Removing the configuration block does fail closed: the
  callback denies with `ConfigDrift` (`federation.rs:1122-1139`). But the
  login screen still offers the provider, so the operator gets a flow that
  fails only at the end. *Corrected 2026-11-12:* the login screen offers no
  provider at all, enabled or not. It has never rendered a federation
  button, and `list_enabled` has no production caller. The architect cited
  that function without tracing its callers. See
  [`096-b2-login-screen-buttons-decision-2026-11-12.md`](096-b2-login-screen-buttons-decision-2026-11-12.md).

**None of this is a hidden vulnerability.** The 7b behaviour is a recorded
interim, and removal fails closed. But it is the gap between what an operator
intends and what sui-id does, in the trust decision itself. That is exactly
what the second half of your philosophy rules out.

## 3 — the proposed cycle

**Track 1 — build 096-B2** (implementation capacity), staged as 096-B1 was:
- **Stage 0, the architect's**: a stage plan written against the code **as built**.
  M2a/M2b's runner, C16 (provider create, already Class-A since M2b wave 3) and
  the RFC 134 reconcile all postdate RFC 096's text, so the plan measures each
  command's real insertion point before anything is dispatched.
- Then C23 (configuration changes are written, and disable), the preflight
  capability and C17, C18, C19/C20, and the admin surface RFC 096 permits:
  *"admin UI may enable or disable but may not mutate trust fields"* (`:380`).
- **The upgrade path is designed and tested, not left to happen** (§4).

**Track 2 — get RFC 100 to acceptance** (design capacity only, nothing built):
the architect refreshes RFC 100 against the seam as built and proposes
answers to its two open questions. The independent review it requires then
comes from you or an external architect (§5, item 3). If it is accepted
during `0.90.0`, M2c is buildable for `0.91.0`, and M6 behind it.

**Also design-only, already agreed**: the Verus scoping note, one minimal
target.

**Not in `0.90.0`**: the store boundary, the embeddable module, RFC 144 and
RFC 145. Each needs your freeze exception, and none is on the path to the
soak.

## 4 — the one consequence you should not be surprised by

**RFC 096 as accepted makes existing providers stop working on upgrade until
an administrator re-enables them.** RFC 096 `:1024-1029`: the migration
*"sets every provider disabled, `validation_status=requires_review`, and
`activation_generation=0`"*. It preserves provider IDs, sealed secrets and
users' links, and reports each provider for the operator. The reason is
stated there too: existing rows *"lack enough durable evidence to infer
endpoint origins, algorithm policy, authentication method, or a safe
preflight"*. That provider-table migration has not run yet (no
`requires_review` column exists today), so it is 096-B2's.

That is the secure reading, and it is already decided. **This scope does not
reopen it.** But it means federated sign-in on an upgraded host **stops** until
someone acts, which is a change no operator should discover by
accident. So
the cycle's acceptance criteria include, at minimum:
- **a start-up warning, logged once, naming each provider** that needs
  re-enabling and how;
- **the login screen does not offer a disabled provider at all**, rather than
  failing at the callback;
- **the admin UI shows the state and the action in one place**;
- **an upgrade note** that leads with this, before anything else in the release.

**If you would rather existing providers stay enabled across the upgrade**,
grandfathered until their next configuration change, that changes RFC 096's
accepted design. It would return to you as an amendment, not a scoping detail.

## 5 — what is yours

1. **The scope**: `0.90.0` = cycle L, **RFC 096-B2**, with RFC 100's
   acceptance as a design track beside it. **Recommended.** The alternatives are
   the store-boundary RFC, or the older-users pair (RFC 144 + RFC 145). Either
   needs your freeze exception, and neither moves the programme toward the soak.
2. **The upgrade consequence (§4)**: confirm that RFC 096's design stands, with
   providers disabled on upgrade until re-enabled. **Recommended**, with the
   four criteria above. Or ask for the grandfathering amendment.
3. **RFC 100's independent design review**: by you, or by an external architect
   you engage (§S1a). The architect prepares the refreshed RFC and a review
   request either way. **Your call alone.**
4. **The ROADMAP relabel**: L becomes 096-B2 (`v0.90.0`), and the
   store-boundary and embeddable rows lose their versions until scheduled.
   That avoids a forward table naming a release for work nobody has
   authorized, which is the stale-plan defect of 2026-11-01. **Recommended.**
