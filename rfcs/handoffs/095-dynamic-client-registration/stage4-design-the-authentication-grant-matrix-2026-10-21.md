# M3 stage 4 — the authentication/grant matrix: design

> ## **Rewritten 2026-10-21.** The first version was wrong twice, and both are recorded rather than quietly replaced.
>
> **1. It dispatched an "independent design review" to the implementation
> role, citing RFC 000 as permitting it.** The implementation role's own
> operating instructions scope it to review *requests*, clarification
> *requests* and risk *reporting* — not verdicts. The arrangement was the
> architect's error, asked for in writing.
>
> *Citation corrected 2026-10-21.* This paragraph cited RFC 000 `:39-41`
> for *"The implementer cannot be the sole approver of a security-sensitive
> design or its closure evidence."* That text was in RFC 000 when the
> correction was written and `@nabbisen` moved it out the same day; its
> normative home is `docs/development-specification.md`. **It also does not
> say what this paragraph implied**: the operative word is *sole*, and the
> sign-off of record was never the implementation role's. What the
> delegation broke is who may review, not the chain of sign-off.
>
> **3. The escalation was wrong too, and this is the one that cost time.**
> The rewritten header below treated "who may perform a design review here"
> as an open question for `@nabbisen`. It was settled on **2026-10-01**:
> [`ROADMAP.md`](../../../ROADMAP.md) §S1a places design review with
> `@nabbisen` or an external architect he engages and **not** the dev team,
> and §S1c prohibits architect self-review unless he asks for it. §E below
> is withdrawn on that basis. The architect read
> `scripts/check-rfc-integrity.py`'s condition 9 comment, which still says
> the question awaits settlement, and trusted it over the ROADMAP — the
> stale-pointer failure this project keeps meeting. See *Errata: the
> `Independent design review` field* in [`rfcs/README.md`](../../README.md).
>
> **2. It was built on the wrong function.** §B, §C2 and §C3 reasoned about
> `sui_id_core::oauth_token::authenticate_client`, which serves
> introspection and revocation — the two endpoints the same document flagged
> as out of scope. The token endpoint's own grant dispatch calls a
> *different*, private function. One row of the old table was not merely
> unverified but backwards.
>
> Both were found by the implementation role's measurements, which are kept:
> `.git-exclude/reviewed/rfc-095-stage4-design-verification-2026-10-21.md`,
> corroboration under RFC 129 `:5` and not approval.

**Written.** 2026-10-21 JST, by the architect. **Rewritten the same day.**
**Independence, stated plainly.** I wrote RFC 095's matrix, the stage plan
and this design, so **this design is not independently reviewed**, and under
RFC 129 `:24` there is no reviewer outside this team to supply one.
Per RFC 129 `:5`, approval in exactly that situation rests outside the
architect's authority. §E has the one question it turns on.
**Normative source.** [`metadata-validation.md`](metadata-validation.md),
§*Authentication/grant matrix* — eight rows, each tested for both
authorization-code and refresh dispatch.
**Measured at.** `1ab7a5a`.

## A — three fields are accepted, validated, echoed, and never persisted

Not one field, as the first version said.

`ClientRow` (`sui-id-store/src/models.rs:294-320`) has no
`token_endpoint_auth_method`, no `grant_types` and no `response_types`. No
migration adds any of them — the `auth_methods` columns in `0006`/`0019` are
**session MFA factors**, an unrelated concept sharing a word. Stage 3 named
`grant_types` and `response_types` as echo-only; it named the fact without
drawing the consequence, and `token_endpoint_auth_method` is in the same
category and was named by nobody.

**RFC 095 `:362-366` is wrong about three of its six nouns.** It states the
C15 insert writes *"…scopes, auth method, grants, response types, consent
policy, and timestamps in one statement."* Scopes and consent policy are
written; auth method, grants and response types are not.

> **Corrected 2026-10-21.** The paragraph above also lists `source`, which
> I treated as written by the insert. It is written by a *second*
> statement — `commands.rs:4102-4103` calls `clients::create_within_tx`
> and then `clients::set_registered_via_within_tx` — and the next sentence
> of the RFC says *"Separate `set_registered_via` or `update_app_identity`
> writes are forbidden in the dynamic path."*
>
> **Corrected again the same day, and this time against the code rather
> than against the RFC.** I proposed repairing the code to match that
> sentence and **withdrew it on measurement.** The two-call shape is the
> registered design, not a drift from it:
>
> - `contracts/write-commands.toml` declares C15's mutation surface as
>   *"guarded `client_registration_token::consume`, `clients::create`,
>   `set_registered_via` in one RFC 094 transaction"*, and **G17 enforces
>   that row**.
> - C11 (`set_registered_via`) is **class `I`** there — an *internal
>   mutation primitive*, `sealed = false`, no audit event.
>   `command-inventory.md` defines class I as an **explicit exclusion from
>   Class A, not a bypass**. So it is deliberately unaudited, by
>   classification, and the doc comments calling it "separately-audited"
>   had it backwards; those are corrected.
> - `create_leaves_registered_via_at_the_table_default`
>   (`commands/tests/runner/client_persistence.rs`) exists **specifically**
>   to fail the edit I proposed, and says so in its own doc comment.
>
> Atomicity — the property *"one statement"* was reaching for — is held by
> the transaction, so nothing is at risk either way. **The RFC sentence is
> what is out of step with the built design, and amending an Accepted
> RFC's text is `@nabbisen`'s.** Of the six nouns, two are written by the
> insert and one by a registered companion write inside the same
> transaction.

**Whether that needs an RFC amendment is §E's question, not this document's
decision.** RFC 095 is Accepted, and RFC 000 makes a material correction to
it an act with its own procedure.

## B — the eight rows, against the function the token endpoint actually calls

```
authorize.rs:423 → exchange_code      (authorization-code grant)
authorize.rs:592 → exchange_refresh   (refresh grant)
     ↳ both call  authenticate_client  (private, authorize.rs:685)
```

`handlers/oauth_token.rs:67`/`:132` are `introspect` and `revoke`; they call
the *public* `oauth_token::authenticate_client` at `:275`. **That function is
not on this matrix's path.** Nothing added to it affects `/oauth2/token`.

The function that is (`authorize.rs:685-706`), in full:

```rust
async fn authenticate_client(client: &ClientRow, secret: Option<&str>) -> CoreResult<()> {
    if !client.confidential { return Ok(()); }
    let stored = client.secret_hash.as_deref().ok_or(…)?;
    let provided = secret.ok_or(…)?;
    crate::password::verify_password(provided, stored).await.map_err(…)
}
```

| # | Auth method | Request | Matrix | Today |
|---|---|---|---|---|
| 1 | `none` | Basic or any secret | **Reject** | **Accepted** — `!confidential` returns `Ok(())` and `secret` is never inspected |
| 2 | `none` | client id only, no secret | Accept with PKCE | Accept |
| 3 | `basic` | one Basic pair | Accept | Accept |
| 4 | `basic` | secret in body, no header | **Reject** | **Accepted** — no channel is recorded or checked |
| 5 | `post` | id+secret in body | Accept | Accept |
| 6 | `post` | Basic present | **Reject** | **Accepted** — same reason |
| 7 | legacy | either, not both | Accept | Accept |
| 8 | any | **Basic and body both present** | **Reject** | **Accepted** — `client_credentials` prefers Basic silently |

**Four rows do not hold, not three.** Row 1 is the correction: the first
version claimed it already held "by the confidentiality check", which is true
of the introspection/revocation function and false here.

**Severity of row 1: low.** A `none` client has no secret; accepting a
spurious one authenticates nobody who could not already authenticate, and
PKCE still applies. It is a conformance gap, not a bypass. **Row 8 is low for
the same shape of reason** — RFC 6749 §2.3.1 forbids presenting two methods,
but an attacker who can set the header already controls the request.

Rows 4 and 6 need the method persisted. Rows 1 and 8 do not: both are
decidable from `confidential` and the request alone.

## C — the design

### C1 — persist the three fields — **settled: nullable, and visible**

Rows 4 and 6 are unreachable without a stored `token_endpoint_auth_method`.
§C4 is unreachable without stored `grant_types`.

**Decision: a nullable column, `NULL` meaning *unrecorded*, exempt from the
C2 check — and the exempt class must be visible to the operator.**

**Why not `client_secret_basic`.** It is RFC 7591's default and stage 2's
default for *new* registrations, and that is exactly why it is wrong here:
applying a default for a value the registrant supplied is not the same act
as applying it to a row where nobody ever supplied anything. Backfilling
`basic` **asserts a fact that was never recorded**, which is the inference
RFC 095 `:352-353` forbids by name for `registered_via` — *"must not infer
provenance from shape, timestamps, names, or missing application URIs"* —
and which stage 6 exists to avoid. It would also break working deployments
silently: an administrator-created client presenting its secret in the body
would start being refused under C2, with an error about a method it never
chose.

**Backfilling from evidence is not available, and nobody should propose it
later.** Measured: `client.dynamic_register` records the client id and name
and no auth method; `confidential` and `secret_hash` establish that a
secret exists, not which channel carries it. There is nothing to derive
from.

**What `NULL` costs, and the half of the decision that is not the column.**
A permanently unchecked class is a real cost and it must not be an
invisible one. The exemption is only defensible if an operator can see it:
a client whose method is unrecorded must *read* as unrecorded wherever
clients are listed — not blank, and not rendered as a guessed value — and
the migration must report how many rows it left that way. A silent
exemption is the failure the second half of this project's standard is
about.

**The class is narrower than it first looks.** `authenticate_client`
returns `Ok(())` for `!confidential` before inspecting any secret, so a
public client has no channel to police. The unchecked class is
**confidential clients registered before the column existed** — nothing
else.

**Scope bound.** Stage 4 adds the column, the `NULL` exemption and the
visibility. It does **not** add an administrator action for recording a
method on an existing client; that is the exit path from the unchecked
class and it is scheduled in §G, not built here.

### C2 — enforce the channel in `authorize::authenticate_client`

Not in the public `oauth_token` one. **There is no shared-function scope
question**, which the first version worried about: nothing but
`exchange_code` and `exchange_refresh` calls this one, so fixing it touches
no code introspection or revocation runs. Whether the same check should also
go to the other function is a genuinely separate decision and is **not** part
of this stage.

> **Measured 2026-10-22, and it is a prerequisite this section did not
> state: the channel never reaches core.** `handlers/oidc.rs:378-401`
> collapses it before calling either grant:
>
> ```rust
> let client_id_raw = header_client.or_else(|| form.client_id.clone())…;
> let client_secret = header_secret.or_else(|| form.client_secret…);
> ```
>
> `CodeExchangeRequest` and `RefreshExchangeRequest` carry the *values* and
> not their origin, so `authenticate_client` cannot decide rows 4, 6 or 8
> today — it has nothing to decide them from. **The HTTP layer must pass
> the presentation down as typed data**, because it is the only layer that
> can see it, and core must keep the decision, because the decision needs
> the stored method and the HTTP layer has no business loading client rows
> to authenticate. A three-state value — Basic, body, or both — carried on
> both request structs. Not a `bool`, and not two.
>
> That same expression is also row 8 in miniature: `.or_else(…)` is what
> *"prefers Basic silently"* means, at `:399`.

**The timing question — settled, and my own framing of it was wrong.**
This section previously said the choice was between *"introducing
uniform-timing handling for the first time"* and *"shipping without it"*.
That is a false pair. **RFC 123 D4 is an Implemented rule that already
governs this code**: *"every branch that rejects before reaching a real
`verify_password` call runs a dummy one first"*, implemented in
`oauth_token::authenticate_client` (`:269-305`) and measured there at ~34ms
hashing against under 200µs unequalised.

`authorize::authenticate_client` has **three** such branches today —
`!confidential`, no stored hash, no provided secret — and a dummy on none
of them. It is not a function without the pattern; it is **the one place on
this path that does not comply with a rule already in force.** A bare
channel check would add a fourth non-compliant branch.

**Decision: add the channel check, and bring this function into RFC 123 D4
compliance in the same change.** The pattern is not being invented — it is
copied from the function next door, with the same decoy and the same
measurement method. Shipping a conformance fix that knowingly deepens a
different conformance violation is the thing to avoid here; the two belong
in one round because they are one function and one test harness.

**The error must still say what went wrong.** A `post` client that presents
Basic gets `invalid_client` as RFC 6749 §5.2 requires, with a description
naming the channel. The registered method is not a secret — the registrant
received it in their own registration response — so a clear description
costs nothing and saves an integrator a debugging session. Vague is not
safer here; it is only vaguer.

**This closes the adjacent finding, so its disclosure becomes a question.**
The property above is the one recorded, unfixed, in
`.git-exclude/reviewed/rfc-095-stage4-design-verification-2026-10-21.md`
§4 and bearing on RFC 123's closure. Once fixed it may be stated publicly,
but **how** it is stated is `@nabbisen`'s: `.github/SECURITY.md` reserves
severity and timeline to him. The dispatch must therefore return the
`CHANGELOG` wording for that half rather than writing it.

### C3 — reject both channels present

If a Basic pair and body credentials are both present, reject rather than
prefer Basic. Needs no column and does not depend on C1, so **it can ship
alone**. §D records that decision and whose it was.

> **Location corrected 2026-10-22.** This section said
> `client_credentials` (`handlers/oauth_token.rs:188`). **That is the
> introspection and revocation helper** — the same wrong-function mistake
> the first version of this whole document made, in a smaller place. Row 8
> belongs to the matrix's own axis, authorization-code and refresh
> dispatch, and that is **`handlers/oidc.rs:378-401`**, where
> `header_secret.or_else(|| form.client_secret…)` at `:399` silently
> prefers Basic. Both-present is detectable exactly there, before anything
> is handed to core.
>
> `handlers/oauth_token.rs:188` is the twin and has the same preference.
> Fixing it is §G item 3's business, not this stage's.

### C4 — the refresh half

*"Refresh is rejected unless both grant policy and scope policy authorize
it."* Sixteen cases, not eight. **This depends on C1**: there is no stored
`grant_types` to check a refresh request against. Scope policy is stored
(`allowed_scopes`) and is checkable today.

## D — the three decisions, and who settled them

**All three are settled and the dispatch carries them**; none is the
implementation role's to reopen.

| | Decision | Settled by |
|---|---|---|
| **C1** | Nullable column, `NULL` = unrecorded, exempt from C2, and the exempt class made visible | The architect, at the owner's request |
| **C2** | Add the channel check **and** bring the function into RFC 123 D4 compliance in the same change | The architect, at the owner's request |
| **C3** | Ships, and may ship independently of C1 | `@nabbisen`, 2026-10-22 |

**The architect settled C1 and C2 because the owner asked him to**, in
those words on 2026-10-22: *"C1, C2: Review it carefully by yourself with
my fundamental philosophy."* That is the `ROADMAP.md` §S1c exception —
self-review is prohibited by default and *"permitted only where he asks for
it, and the record must say he asked"*. This row is that record.

**C3**, in his words the same day: *"Yes, if required and reasonable."* It
is required — row 8 does not hold and RFC 6749 §2.3.1 forbids presenting
two methods — and reasonable, since it needs no column and no migration.

## G — what to record, prepare for, or schedule beyond this stage

Asked for directly, 2026-10-22. Six items; none blocked stage 4, the first
two are created by its own decisions, and item 3 was added after stage 4b
landed.

1. **An exit path from C1's unchecked class.** Nullable leaves confidential
   pre-column clients permanently unverified against their channel. An
   administrator action that records a method on an existing client empties
   that class one row at a time. It is a new durable write, so it needs an
   inventory row and a class under RFC 094 — real work, not a flag.
   **Schedule: `0.84.0` or later. Not stage 4.**
2. **Whether the column should ever become required.** Only answerable once
   item 1 exists and the class is observably empty on a real deployment.
   Recording the question now so that "make it NOT NULL" is never proposed
   as a tidy-up before the exit path exists. **No date; gated on item 1.**
3. **Finish RFC 123 D4 on the token endpoint, and consider a per-client
   bucket.** Stage 4b equalised every rejecting branch *inside*
   `authorize::authenticate_client`. Its two callers still return before
   it — unknown client, and disabled or deleted — so the endpoint's cost
   is not yet uniform. **The scoping was the architect's**: the dispatch
   named a function when the property belongs to an endpoint, the same
   mistake stage 4's first design made.

   > **Corrected 2026-10-24, and the correction shrinks this item.** This
   > entry said *"`/oauth2/token` has no counterpart to RFC 123's per-IP
   > and per-client limiters"*. **False.** `handlers/oidc.rs:371-377`
   > enforces `RateLimitKey::Token` as the handler's very first statement,
   > before credentials are even parsed, at **60 requests per 60 seconds
   > per IP** (`runtime/ratelimit.rs:134`). What the endpoint lacks is the
   > *per-claimed-client-id* bucket, the second half RFC 123 D2 added
   > precisely because per-IP alone misses a distributed source
   > (`ratelimit.rs:123`). So the gap is one bucket, not two, and the
   > load argument against the ~34 ms dummy hashing largely goes with it:
   > 60 probes a minute from one address is roughly two seconds of
   > blocking-pool time, not a saturation risk. The architect asserted the
   > absence without grepping for the limiter.

   **Settled and dispatched 2026-10-24** —
   [`../123-authenticating-a-client-costs-the-caller/d4-on-the-token-endpoint-2026-10-24.md`](../123-authenticating-a-client-costs-the-caller/d4-on-the-token-endpoint-2026-10-24.md).
   Both decisions are the architect's, at the owner's request under
   `ROADMAP.md` §S1c: equalise cost **and** response for every
   pre-authentication failure, and build **no** per-client bucket — RFC
   123 D3's own test says a public `client_id` must not become a lockout
   key, and the blast radius on this endpoint is every user's login.
4. **The same channel check on `oauth_token::authenticate_client`**
   (introspection and revocation). §C2 notes it is a separate decision and
   this stage does not touch it. Those endpoints have their own RFC 7009 /
   7662 semantics and deserve their own measurement. **Schedule: with
   M2b's auth-path tranche, not before.**
5. **RFC 095 `:362-366`'s erratum** — the sentence forbidding a separate
   `set_registered_via` write in the dynamic path, which the RFC 094
   Stage-0 classification contradicts and G17 enforces against. The text is
   the thing out of step. **`@nabbisen`'s, no date.**
6. **The two gate comments describing the self-review question as open** —
   `scripts/check-rfc-integrity.py` condition 9 and its twin in
   `scripts/tests/test_rfc_integrity.py`. §S1c asked for this on 2026-10-01
   and it is still outstanding; it cost this engagement a wrong escalation
   and will cost the next one the same. **`@nabbisen`'s, and the cheapest
   item on this list.**

## E — withdrawn: the question was already answered

> **Withdrawn 2026-10-21.** This section asked *"Who may perform a design
> review here?"* as an open question. It was settled on **2026-10-01** and
> the architect did not find it.

[`ROADMAP.md`](../../../ROADMAP.md) §S1a sets out the workflow in the
owner's own words from that day, and its design-review row reads:
**`@nabbisen`, or an external architect he engages. Not the dev team.**
The table is not reproduced here — §S1a is the record, and copying a
governance table into a handoff is how a restatement drifts from its
source.

§S1c adds that architect self-review is **prohibited by default** and
permitted only where he asks for it, with the record saying he asked.

**So this design's review is not the architect's to conduct either**, absent
his asking. Two statements elsewhere in this document are corrected by that:
the header's *"corroboration under RFC 129 `:5`"* framing, and the claim
below that this design can proceed on disclosed non-independence. Neither is
the architect's call to make here.

**Why it was escalated anyway.** `scripts/check-rfc-integrity.py` condition
9's comment still says the check must not be added until self-review has
been settled — three weeks after it was. §S1c itself asks for that comment
to be brought up to date and it never was. The architect read the gate
source, took the question for open, and raised it a second time. The
pointer was stale; the record was not.

**What is genuinely outstanding on this design**, and it is the ordinary
gate rather than a governance question: `@nabbisen` reviews it, or engages
an external architect to. §A–§D stand as written and are measured.

**Stage 4 is not dispatched and nothing waits on the implementation role.**
