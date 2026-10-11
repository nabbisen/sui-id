# RFC 096-B1 — stage plan revision — proposal

**Written.** 2026-10-11 JST, by the architect.
**Status. Proposal. Not in effect.** The authorized stage order remains
[`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md) until this
revision has a go-ahead. Stage 6 is **not dispatched** in the meantime, for the
reason in §4.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Prior.** Stages 0–5 complete and committed (`8a8dec4` is stage 5).

## 1. What this corrects, and whose error it was

The authorized plan has eight stages, 0–8, and said of itself: *"Checked
against `:23` before stage 1, not after stage 9."* That check was real but
incomplete. `:23`'s 096-B1 clause names six closure items and the plan maps all
six. **`:89-93` names more, and the plan does not map it:**

> Requires RFC 094 **M2a foundation** […] plus the F01–F06 federation login
> commands, which **this stage implements** against that seam (F04 on the
> Class-A runner; F01, F02, F03, F05, F06 on the Protocol runner).

The plan's stage 6 reads *"session establishment (F01, F02 and F04; F04 is the
Class-A one)"*. **Three of the six commands RFC 096 assigns to 096-B1 — F03,
F05 and F06 — are assigned to no stage at all.** F03 and F05 were named in the
plan's own 2026-10-10 correction, where they were correctly identified as *not*
being stage 2's or stage 3's, and then not placed anywhere. F06 is not
mentioned in the plan at all.

Four further requirements are unassigned, found by reading the sections those
commands live in rather than the command list:

| RFC 096 | requirement | assigned |
|---|---|---|
| `:728-738` | the `federation_mfa_pending` and `federation_webauthn_ceremony` migrations, with the 0–5 failure constraint, the four-state status, `expires_at = created_at + 300`, the unique parent-pending key and the no-later-expiry rule | nowhere |
| `:752-765` | `FederatedMfaVerifier` — binding before anything else, sealed non-cloneable method candidates, closed rejection reasons selectable only by the verifier | nowhere |
| `:605` | *"Completed/failed/expired attempts are periodically deleted after 24 hours."* | nowhere |
| `:600-601` | *"The callback cookie is expired on every terminal path."* | nowhere |

This is the same class of error as 096-A's: a stage list checked against the
closure sentence rather than against every clause the stage owns. It is the
architect's, found before dispatching stage 6 rather than after stage 8 — which
is the only part of it that went right.

**`:605` has a consequence in `main` today.** `federation_login_attempt` has
`insert`, `find_by_state_sha256` and `claim`, and no delete of any kind;
`runtime/gc.rs` purges six tables every fifteen minutes and that is not one of
them. Rows therefore accumulate without bound from stage 2 onward. **No
released version is affected** — stages 1 and 2 landed after `0.79.0` — and §5
makes it a release precondition rather than leaving it to the stage that
happens to reach it.

## 2. The revised stage order proposed

Stages 0–5 are complete and unchanged. From stage 6:

**Stage 6 — F05, F01, and the two retention items.** Terminal attempt failure
first, because `:602-604` requires every failure path to leave the row
reconcilable as `failed` and today none of them does; then F01, the
existing-link, no-local-MFA completion — guarded `exchanging -> completed`,
observe last-seen and bounded verified email, login bookkeeping, `[Fed]`
session, session cap. With them, the two retention requirements that belong to
the attempt row rather than to a command: `:605`'s 24-hour purge, registered in
`runtime/gc.rs` beside the existing six, and `:600-601`'s cookie expiry on
every terminal path. Both Class P. **This is the stage after which `main` is
releasable** (§5).

**Stage 7 — F04, the Class-A one.** First federated provisioning and direct
login in one `WriteTx<AtomicAudit>` transaction: guarded attempt completion,
recheck of absent link, verified unique email and username uniqueness, a
passwordless non-admin user, exactly one provider/sub link, a `[Fed]` session,
the cap, and the sole `auth.federation.provisioned` intent event. Federation's
first use of the Class-A runner, so this stage needs the heaviest review of the
remainder. Carries the three items stage 5's review left to it: the `L04` audit
event's upstream `sub` attribute and the new event's own, the
`provision_on_first_login` sentence in `docs/src/guides/operators.md` becoming
true, and the duplicate `federation_link::find_by_sub` per callback collapsed.

**Stage 8 — migration `0047`: `federation_mfa_pending` and
`federation_webauthn_ceremony`.** Schema only, `:728-738` exactly. Migration
evidence is a `:23` closure item, so it is produced here rather than
reconstructed later — the same reasoning as stage 1.

**Stage 9 — F02.** Existing link, local MFA required: guarded attempt
completion plus exactly one pending row sealed as `Fed` primary and bound to
provider, version, activation generation, link, user and continuation. No
login-success event before MFA (`:719-726`).

**Stage 10 — `FederatedMfaVerifier` and F03.** The verifier's
binding-before-anything entry point and its sealed candidates (`:752-765`), and
F03's one Protocol transaction with the failure-count semantics at `:741-751`:
1–4 is `RejectedStillPending`, exactly 5 is `AttemptsExhausted` and destroys
the continuation, no sixth increment exists, and whichever guarded transaction
serializes first determines the equality boundary. That boundary needs a test
of its own, in the shape stage 3's claim boundary got.

**Stage 11 — F06.** The federated WebAuthn ceremony: atomic at-most-one
replacement bound to parent pending row, RP, origin and challenge, with capped
expiry and no later expiry than its parent.

**Stage 12 — remove the shipped defect's two paths.** The trust-on-TLS decode
and the cookie-replay path, plus the two discovery items 096-A could not wire —
the configured-value cross-checks and `validate_discovery_bytes`'s tightened
bounds. Unchanged from the authorized plan's stage 7, except that it now comes
after the MFA continuation exists: removing the legacy path takes the generic
pending-MFA mechanism with it, so F02/F03/F06 must be in place first. Also
carries `:1067`'s slug-rather-than-internal-id audit note.

**Stage 13 — federation failure surfacing.** *Separately proposed, and listed
here so the order is complete.* No handler or template reads the `fed_error`
query parameter, so all twenty-seven federation failure values land a user on
an unexplained login page. The work is a bounded map onto a small set of
user-visible messages, not a rendering of the internal value. **If this is not
taken up, stage 13 drops and live-integration evidence becomes stage 13.**

**Stage 14 — representative live-integration evidence.** One public upstream,
real credentials, outside CI, secrets absent from artifacts (`:1021`). A `:23`
closure item. Unchanged from the authorized plan's stage 8.

## 3. What this does not change

- **The stages already complete.** Nothing in 0–5 is reopened.
- **096-B2's boundary.** C17/C18/C23 remain 096-B2, blocked on RFC 094 M2b,
  which is unstarted.
- **096-A's acceptance**, which is separate from all of this.
- **The ordering principle.** One dispatch at a time, one command per stage.
  That is what has made each package reviewable, and the five stages since the
  plan was authorized have not given a reason to bundle.

## 4. Why stage 6 is not dispatched yet

Under the authorized plan, stage 6 is F01, F02 and F04 together. F02's whole
output is a pending MFA row in a table that does not exist, to be consumed by
F03, which is in no stage. Dispatching it would mean commissioning a writer
with no schema and no reader — the shape of defect the last two rounds have
been spent finding, manufactured deliberately. So the dispatch waits on the
revised order rather than going out in a form already known to be wrong.

## 5. Release schedule

The authorized plan assumed one release at the end of 096-B1. With seven stages
remaining rather than two, that is a long runway, and stage 5's fix round
landed corrections in `main` (`8a8dec4`) that are worth operators' hands sooner
than that. The findings behind them are recorded in this round's review result
under `.git-exclude/`, which is also where the severity and disclosure
discussion belongs until `0.80.0` ships (`.github/SECURITY.md`).

**Proposed instead: cut `0.80.0` after stage 6**, then continue 096-B1 toward
`0.81.0`.

- Stage 6 is the point at which `main` stops accumulating attempt rows
  (`:605`), which is the one thing in `main` that should not ship.
- Everything stage 5 and its fix round landed — the generic `link_only`
  result (`:699`), `:714-715`'s log and audit changes, the configuration and
  operator guide corrections — reaches operators then rather than after the MFA
  continuation, the Class-A provisioning commit and the legacy-path removal.
- No half-built user-visible surface ships: `/auth/federated/link`'s removal
  takes away a skeleton that never created a link, and both guides now say so.
- The new resolution is computed but not authoritative at stage 6 for anything
  except the paths F01 and F05 own, so the release is coherent rather than
  mid-rewrite.

**A `0.79.1` patch release is considered and not recommended, on mechanics.**
`main` is 196 commits and 242 changed files past the tag,
`handlers/federation.rs` among them heavily, and this round's regression tests
rely on a TLS mock that postdates the tag — so it would be a hand-written patch
with hand-written tests on a release branch, not a cherry-pick. The weighing of
that cost is in the review result rather than here; the recommendation is to
reach stage 6 and cut `0.80.0` instead.

Both the revised order and the release checkpoint need a go-ahead before
either takes effect.
