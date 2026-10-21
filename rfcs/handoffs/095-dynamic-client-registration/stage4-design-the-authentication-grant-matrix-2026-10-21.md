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

### C1 — persist the three fields, or decide not to

Rows 4 and 6 are unreachable without a stored `token_endpoint_auth_method`.
§C4 is unreachable without stored `grant_types`.

**The migration default is the real decision.** Existing rows record no
method. `client_secret_basic` is RFC 7591's default and stage 2's, but it
retroactively asserts a method for every pre-column client, and an
administrator-created client using `post` would begin to be rejected under
C2. A nullable column where `NULL` means *unrecorded*, exempt from the C2
check, cannot break a working deployment on upgrade but leaves a permanent
unchecked class. **I incline to nullable and I am not ruling it** — see §E.

### C2 — enforce the channel in `authorize::authenticate_client`

Not in the public `oauth_token` one. **There is no shared-function scope
question**, which the first version worried about: nothing but
`exchange_code` and `exchange_refresh` calls this one, so fixing it touches
no code introspection or revocation runs. Whether the same check should also
go to the other function is a genuinely separate decision and is **not** part
of this stage.

**One consequence to decide explicitly, not inherit.** The first version
said "follow the existing `DUMMY_PHC` pattern; it is there for this reason."
That pattern is in the *other* function. **This one has none.** So adding a
channel check here either introduces uniform-timing handling for the first
time — a larger change than "follow the existing one" — or ships without it.
Which, and why, is an answer this stage owes before it is dispatched.

*A related property of this function, unfixed, is recorded in
`.git-exclude/` rather than here: it concerns an existing closure
prerequisite, `.github/SECURITY.md` reserves severity and timeline to the
owner, and the repository is public.*

### C3 — reject both channels present

In `client_credentials` (`handlers/oauth_token.rs:188`): if a Basic pair and
body credentials are both present, reject rather than prefer Basic. Needs no
column and does not depend on C1, so **it can ship alone** if C1 needs more
deliberation than this stage can carry.

### C4 — the refresh half

*"Refresh is rejected unless both grant policy and scope policy authorize
it."* Sixteen cases, not eight. **This depends on C1**: there is no stored
`grant_types` to check a refresh request against. Scope policy is stored
(`allowed_scopes`) and is checkable today.

## D — the implementation questions, for the dispatch

Not for the implementation role to rule on now; these are what the eventual
dispatch must settle and state: the C1 default, whether C2 introduces
uniform timing, and whether C3 ships independently.

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
