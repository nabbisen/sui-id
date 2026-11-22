# RFC 096-B1 — completion assessment

> ## Accepted 2026-11-23
> `@nabbisen`: *"Accept 096-B1 (Recommended)"*. 096-B1 is closed, and with
> it ROADMAP milestone **M4-B**. RFC 097 (M5) starts.

**Written.** 2026-11-22, by the architect.

**Not independent.** The architect wrote 096-B1's stage plan and every
dispatch, folded code into every stage, ran both live runs, and writes
this. This is the same position as 096-A's and 096-B2's assessments, where
RFC 000's rule for when no independent role exists applies.

**The rule.** RFC 096, *Closure prerequisites*, 096-B1, names six kinds of
evidence:
- durable attempt state;
- one-time nonce consumption;
- session establishment;
- removal of the trust-on-TLS and cookie-replay paths;
- migration evidence;
- representative live-integration evidence.

The table below takes them in that order.

**The plan was checked against that rule first, not last.** See
[`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
*Checked against `:23` before stage 1*. Every clause has a stage. Every
stage landed under Level B, and its commit is cited below and checked with
`git cat-file`.

## Clause by clause

| The rule requires | Built by | Commit | What it is |
|---|---|---|---|
| **Durable attempt state** | stage 1 | `0ed0db5` | migration 0046, `federation_login_attempt` |
| | stage 2 | `226d1a6` | the attempt row written before the redirect (F07) |
| | stage 6a | `3c76c6c` | attempt retention, and the cookie cleared on every terminal outcome |
| | stage 6c | `87a52aa` | the attempt state machine, and F05 |
| **One-time nonce consumption** | stage 3 | `d8e10d4` | the atomic claim; the nonce consumed once |
| **Session establishment** | stage 4 | `d16b4c3` | the live callback validates (096-A's pipeline gets its caller) |
| | stage 5 | `8a8dec4` | verified-identity mapping |
| | stage 7 | `e9d5001` | F04, first federated provisioning, in one transaction |
| | stage 9 | `f5b4035` | F02, the first federated MFA continuation |
| | stage 10b | `f3e6925` | F03, the four guarded outcomes |
| | stage 11 | `749571b` | F06, and the third method, end to end |
| | stage 6d | `a6524e0` | F01, Class A, folded into L04 |
| | stage 12b | `09c1e04` | the federated MFA submission gets its caller |
| **Removal of the trust-on-TLS and cookie-replay paths** | stage 12a | `57b4ead` | the verified pipeline becomes the one that acts; the legacy unverified decode is deleted |
| | stage 12c | `744e699` | the token endpoint auth method; with 12a, both legacy paths are gone |
| | `0.92.0` N2d | in `0.92.0` | the userinfo fallback removed: the ID token is the only identity source (RFC 096) |
| **Migration evidence** | stage 1 | `0ed0db5` | migration 0046 |
| | stage 8 | `d7d5523` | migration 0047, the two MFA-continuation tables |
| **Representative live-integration evidence** | stage 14 | on `c30056b` | [Duende demo record](096-b1-stage14-live-run-record-2026-11-18.md): the security path, refusal, disable, delete |
| | the M4-B run | on `67f1140` | [Google record](096-b1-m4b-google-run-record-2026-11-22.md): **steps 1–12 passed with no patch**, including account creation from the ID token, the user page, unlink, and the takeover refusal |

Release surfacing, which the plan made a release precondition rather than
a closure clause, also landed: stage 6e (`92b903e`) renders `fed_error`, and
says plainly what `link_only` means.

## What the live runs added, and fixed, beyond the clauses

The two live runs did what tests could not, in four places:

- **Duende, stage 14.** Its client sends the email from userinfo, not the
  ID token, so account creation was refused. That was by design (RFC 096),
  and the refusal was made honest in `0.92.0` N2d. It is why M4-B waited
  for a second upstream.
- **Google, first run.** sui-id refused Google's signing-key host, which
  answers with 16 DNS addresses against a cap of 8. **As built, it could
  not federate with Google at all.** Fixed in **O6** (`67f1140`): the cap
  is now 32 in total, with every address still checked. RFC 096 is
  amended. O6 also made refusals name their cause, and made destructive
  pages state their consequence plainly. That came from the owner's own
  observations during the run.
- **Google, the record run.** A federation-only user left without any way
  to sign in, after an unlink, led to measuring **RFC 103 T10**. It had
  lapsed for federated accounts: F04 (stage 7) stores them with
  `source = local`, and the recovery-link check read only `source`. Fixed
  in **O7** (`f9b81ad`): the check is made at issue, at completion and at
  password sign-in. It is disclosed under CHANGELOG *Security*, and was
  held outside git until it landed
  ([finding](0.93.0-o7-the-t10-finding-2026-11-22.md)).
- **The unlink page** now says what remains after an unlink, and what to do
  when nothing does (O7).

**This is the evidence working as intended, not a reason to doubt the
clauses.** Each defect sat outside what a clause names: transport bounds,
a different RFC's control, wording. Each was found by the run that
096-B1's last clause exists to require, and fixed before this assessment.

## Residuals, stated, not open defects

- **No re-link.** An administrative unlink cannot be undone from the
  admin UI (C19 was withdrawn by amendment A1); the page says so.
- **Upstream re-authentication for step-up is not built.**
  `step_up::first_factor_proof` returns `Unavailable` for a federated
  account, so enrolment that needs a fresh first factor is refused.
- **A federated user's email is not locally verified** (RFC 101 D1: an
  upstream's claim is not proof of control here). RFC 101's build verifies
  it.
- **Representative, not exhaustive.** Two upstreams, Duende and Google,
  ran live. RFC 096 asks for one representative public integration, so
  this is past the bar, not a matrix of providers.
- **Carried into cycle P** (RFC 149), from the owner during the run: the
  footer's noise on every page, and a short localized label for the DNS
  deny reason.

## Recommendation

**Accept 096-B1.** Every clause is met by landed, Level-B stages. The last
clause is met by a run on the unpatched current code, against Google.
Everything the runs found is fixed and landed, or carried to cycle P
as the record shows. Acceptance closes M4-B, and RFC 097 (M5) starts.
