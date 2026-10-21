# M3 stage 4b — enforce the authentication/grant matrix

> ## Whose turn it is: **the implementation role's.**
> Nothing here waits on `@nabbisen` **except one `CHANGELOG` paragraph**,
> named in §9. This is the security-bearing stage: a wrong accept is an
> authentication bypass, not a validation gap. Measure, do not infer.

**Dispatched.** 2026-10-22 JST, by the architect.
**Design.** [`stage4-design-the-authentication-grant-matrix-2026-10-21.md`](stage4-design-the-authentication-grant-matrix-2026-10-21.md)
— §B, §C2, §C3, §C4, §D. **Read §B's table and §C2's two inset blocks
before anything else.**
**Decisions.** Settled; §D records which was whose. Do not reopen them.
**Stage 4a's review.** `.git-exclude/reviewed/rfc-095-stage4a-persist-three-fields-2026-10-22.md`
**Baseline.** Read the tip with `git log -1`.

## 0 — two prerequisites this stage cannot be correct without

Both were measured on 2026-10-22. Neither is optional and neither is work
you should discover halfway through.

### 0.1 — the channel never reaches core

`handlers/oidc.rs:378-401` collapses it before either grant is called:

```rust
let client_id_raw = header_client.or_else(|| form.client_id.clone())…;
let client_secret = header_secret.or_else(|| form.client_secret…);
```

`CodeExchangeRequest` (`authorize.rs:372-379`) and `RefreshExchangeRequest`
(`:381-389`) carry `client_secret: Option<String>` and nothing about where
it came from. **Rows 4, 6 and 8 are undecidable in core as the code
stands.** §2 is the fix.

### 0.2 — an unparseable stored method currently reads as *unrecorded*, and this stage would exempt it

`repos/clients.rs:61-63`:

```rust
token_endpoint_auth_method: auth_method_str
    .as_deref()
    .and_then(crate::models::TokenEndpointAuthMethod::parse),
```

`parse` returns `None` for any string outside the closed three
(`models.rs:328-335`). So a column holding `"CLIENT_SECRET_BASIC"`, or a
value a later version writes, or anything a restore or a hand-edit leaves,
becomes `None` — **identical to "never recorded", which §3 exempts from
enforcement.** That is a fail-open: enforcement silently switches off for
exactly the row whose data is wrong.

The enum's own doc comment notices the ambiguity and judges it benign. **It
was benign for 4a, which only stored and displayed. It is not benign here.**

**Required: unrecorded and unparseable must be distinguishable at the
enforcement site, and unparseable must fail closed.** Two constraints on how:

- **Listing a client must not break.** Erroring out of `map` would mean one
  corrupt row makes `clients::list` fail, so an operator could not even see
  the problem in order to fix it. That is the wrong trade for an
  operational tool.
- **The admin UI should be able to say so**, the same way §5 of 4a made
  "unrecorded" visible. A value nobody can parse is worth more alarm than
  one nobody recorded, not less.

**I incline to an `Unknown(String)` variant on `TokenEndpointAuthMethod`** —
`None` keeps meaning unrecorded, `Some(Unknown(_))` means present and not
understood, nothing else moves, and `as_str`'s `&'static str` return is the
one thing that needs adjusting. **Build what you judge best and say what you
built and why.** Through the application this state is unreachable —
`validate_auth_method` is the only writer and the model is an enum — so this
is a guard against a database that was changed from outside, which is
precisely when an authentication system should refuse rather than relax.

## 1 — what must hold when this lands

§B's table, all eight rows, **each tested for both `exchange_code` and
`exchange_refresh`**. Four do not hold today:

| # | Auth method | Request | Must be |
|---|---|---|---|
| 1 | `none` | Basic, or any secret | **Reject** — decidable from `confidential` alone, no column needed |
| 4 | `basic` | secret in body, no header | **Reject** — needs 4a's column |
| 6 | `post` | Basic header present | **Reject** — needs 4a's column |
| 8 | any | Basic **and** body both present | **Reject** — no column needed, §5 |

The other four already hold and must still hold; assert them rather than
assume them.

## 2 — the credential presentation, typed

The HTTP layer is the only place that can see the channel, and core must
keep the decision, because the decision needs the stored method and the
HTTP layer has no business loading client rows to authenticate.

A **three-state** value — Basic, body, or both — on both request structs,
carried from `handlers/oidc.rs`'s existing observation at `:378-401`. It
replaces nothing: `client_secret` stays, because the secret's *value* and
the secret's *channel* are different facts.

**Not a `bool`, and not two booleans.** Two booleans make `(false, false)`
and the "neither" case representable twice, and a `bool` cannot carry row
8 at all. The state where no credential arrived is the `Option` on
`client_secret` that already exists — do not duplicate it into the new
type, and say in the type's own doc comment which fact lives where.

## 3 — the channel check

In `authorize::authenticate_client` (`authorize.rs:685-706`). **Not** in
`oauth_token::authenticate_client` — that one serves introspection and
revocation and is out of scope (§G item 3 of the design).

- **`NULL` / unrecorded → exempt.** C1's decision. The row is unchecked and
  that is deliberate; 4a made it visible.
- **Unparseable → reject.** §0.2.
- **`none` → reject any presented secret**, from either channel. Row 1.
  Note this is decidable without the column, from `!confidential`, and
  §B judges it **low** severity: a public client has no secret, so
  accepting a spurious one authenticates nobody who could not already
  authenticate, and PKCE still applies. Fix it because it is a conformance
  gap, and do not dress it up in the `CHANGELOG` as more.
- **`basic` → the secret must have arrived in the header.** Row 4.
- **`post` → the secret must have arrived in the body, and no Basic header
  may be present.** Row 6.

## 4 — RFC 123 D4, which this function does not currently obey

This is the part most likely to be skipped because it looks like someone
else's job. It is not.

**The rule, already Implemented:** *"every branch that rejects before
reaching a real `verify_password` call runs a dummy one first"* —
`oauth_token.rs:268-273` states it and `:284-305` implements it, measured
at ~34ms hashing against under 200µs unequalised.

`authorize::authenticate_client` has **three** such branches today and a
dummy on none of them: `!confidential` returning `Ok(())`, the missing
stored hash, and the missing provided secret. **Your channel checks would
add more.** Bring the whole function into compliance in this round — copy
the pattern from next door, with the same `password::DUMMY_PHC`.

**One case needs thought rather than copying**: `!confidential` currently
returns **`Ok(())`**, not an error. A dummy hash on a success path costs
every public client 34ms on every token exchange, for no gain — the
rule is about *rejecting* branches. Decide what the right behaviour is,
state it, and do not let "apply the pattern uniformly" turn into a
performance regression on the public-client happy path.

**Measure it.** RFC 123's own evidence is a timing measurement, not an
assertion that the code looks right. Produce the equivalent: the wall-clock
cost of each rejecting branch against a wrong-secret rejection, in a
throwaway benchmark you do not ship, with the numbers in the package.

## 5 — both channels present

At `handlers/oidc.rs:397`, where `header_secret.or_else(|| form.client_secret…)`
is what *"prefers Basic silently"* means. If a Basic pair **and** body
credentials are both present, reject. RFC 6749 §2.3.1 forbids presenting
two methods.

This needs no column and does not depend on anything else here. That is
the footing the design's §D records it on, and it is still part of this
round.

`handlers/oauth_token.rs:188`'s `client_credentials` has the same
preference for introspection and revocation. **Leave it** — §G item 3.

## 6 — the refresh half, which is two independent things

§C4: *"Refresh is rejected unless both grant policy and scope policy
authorize it."* Measured, those are two gaps, not one:

**Grant policy — not checked at all.** `exchange_refresh` never consults
the client's `grant_types`. With 4a's column it can: a client whose stored
grants omit `refresh_token` must be refused. Unrecorded is exempt, as
everywhere else here.

**Scope policy — not re-checked.** `exchange_refresh` issues from
`preliminary.scope` (`authorize.rs:632`, and again at `:644` for the
successor row) — the scope stored on the refresh-token row at issuance, and never compares it to the client's **current**
`allowed_scopes`. So narrowing a client's scope policy does not take effect
on refresh tokens already outstanding. `enforce_scope_policy`
(`authorize.rs:273`) is the existing check and handles the legacy
empty-policy "permit any" case already.

**This second one is a behaviour change operators will feel**, and it is
the one to be careful about: a deployment that narrowed a policy and relies
on existing refresh tokens continuing to work will see them start failing.
That is the policy finally taking effect, which is the point — but it must
be in the `CHANGELOG` in those terms, not as a bug fix nobody reads.

## 7 — what the client is told

`invalid_client`, as RFC 6749 §5.2 requires, **with a description naming
the channel**. The registered method is not a secret — the registrant
received it in their own registration response — so a clear description
costs nothing against an attacker and saves an integrator a long debugging
session. Vague is not safer here; it is only vaguer.

Refresh-grant and scope refusals use their own correct codes:
`unauthorized_client` for a grant the client may not use, `invalid_scope`
for a scope outside policy. Do not collapse all four into one.

## 8 — tests

**Sixteen cases, not eight.** Every row of §1's table against both
`exchange_code` and `exchange_refresh`. A row that holds today still gets
its assertion.

**The unparseable case** from §0.2: write a value outside the closed set
directly into the column, and assert the token endpoint **refuses** rather
than exempting. This is the fail-open test and it is the one a future
refactor is most likely to silently break.

**The unrecorded case**: a client with `NULL` authenticates exactly as it
does today, both grants. 4a's e2e proved one path; this stage must keep it
true under the new code.

**Refresh**, both halves of §6, separately: a client whose stored grants
omit `refresh_token`; and a client whose `allowed_scopes` was narrowed
after a refresh token was issued.

**Timing**, per §4 — numbers in the package, benchmark not shipped.

**Mutation evidence, four, and no more.** One per failing row: 1, 4, 6, 8.
For each, remove that row's check and show its test — and only its test —
failing. If removing one check fails several tests, say so; that means the
cases are not independent and I want to know.

## 9 — what to return

The usual package, plus:

- **What you built for §0.2**, and why that shape.
- **The `!confidential` decision** from §4 and its reasoning.
- **The timing numbers** per rejecting branch.
- **Whether any of the four already-holding rows turned out not to hold**
  once you asserted it rather than assumed it.
- All four mutations.
- The full matrix; Level B scope. Report it local.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>`.

**`CHANGELOG`: `### Security` for the matrix rows and `### Changed` for the
refresh scope re-check** — the first is an authentication conformance fix
on the token endpoint, the second is operator-visible behaviour. Lead the
`### Changed` entry with what breaks, not with what is now correct.

**One paragraph you must not write.** §4 closes the RFC 123 D4 gap recorded,
unfixed, in `.git-exclude/reviewed/rfc-095-stage4-design-verification-2026-10-21.md`
§4. `.github/SECURITY.md` reserves severity and timeline to `@nabbisen`.
**Write the entry for everything else, leave that one as a marked
placeholder, and say so in the package.** I will put the wording to him.

## 10 — what this stage does not do

Not `oauth_token::authenticate_client` (§G item 3). Not an administrator
action for recording a method on an existing client (§G item 1). Not making
the column required (§G item 2). No change to `registered_via`, to the C15
transaction's shape, or to stage 4a's storage decisions.

**This is the last unbuilt piece of `0.83.0`.** When it lands and is green,
the release cut is the architect's and `@nabbisen`'s.

Leave the working tree uncommitted. One open dispatch at a time; this is it.
