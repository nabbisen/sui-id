# M3 stage 4 — the authentication/grant matrix: design, for independent review

> ## Whose turn it is: **the implementation role's — to review this, not to build it.**
> My plan §E reserved an independent design review for this stage before it is
> dispatched, because it is client authentication: the one stage here where a
> wrong accept is a bypass rather than a validation gap. **Attack this
> document.** Implementation is dispatched only after the review returns.

**Written.** 2026-10-21 JST, by the architect.
**Reviewer.** The implementation role — genuinely independent in the sense
RFC 000 allows: I wrote RFC 095's matrix, this plan and this design, and did
not review any of them. Same arrangement as RFC 107's design review.
**Normative source.** [`metadata-validation.md`](metadata-validation.md),
§*Authentication/grant matrix* — eight rows, each to be tested for both
authorization-code and refresh dispatch.
**Measured at.** `2bdc6af` (stage 3, Level B 27/0).

## A — the finding that reshapes this stage

**`token_endpoint_auth_method` is never persisted.** Measured:

- `ClientRow` (`sui-id-store/src/models.rs:294-320`) has no such field. The
  only authentication-shape signal it carries is `confidential: bool`.
- No migration adds the column. `grep` over every `migrations/*.sql` for
  `auth_method` on `clients` returns nothing — the `auth_methods` hits are
  **session** MFA factors from `0006`/`0019`, a different concept that merely
  shares a word.
- The token endpoint never reads it. `client_credentials`
  (`handlers/oauth_token.rs:188`) tries Basic, falls back to the form body.
  `authenticate_client` (`sui-id-core/src/oidc/oauth_token.rs:275`) checks
  `confidential`, disabled, deleted and the secret hash. Neither consults a
  registered method, because there is none to consult.

So the field is **accepted** at registration, **validated** by stage 2 and
**echoed** by stage 3 — recomputed from the request each time, never read
back. Stage 3 correctly identified `grant_types` and `response_types` as
echo-only with no backing column; this field is in the same category and
was not named.

**RFC 095 `:362-366` says otherwise.** Its C15 paragraph states the insert
writes *"…logout URIs, scopes, **auth method**, grants, response types,
consent policy, and timestamps in one statement."* That describes something
that does not happen. **I am not proposing an RFC amendment in this
document** — the RFC is Accepted and amending it is its own act — but the
review should say whether it thinks one is needed.

## B — what this does and does not mean

**It is not a live vulnerability, and I want to be exact about that rather
than dramatic.** Walking the eight rows against today's code:

| Auth method | Request | Matrix | Today | Enforceable without the column? |
|---|---|---|---|---|
| `none` | Basic or any secret | Reject | **Rejected** — `authenticate_client` refuses `!confidential` | n/a, already holds |
| `none` | client id only, no secret | Accept with PKCE | Accept | n/a |
| `basic` | one Basic pair | Accept | Accept | n/a |
| `basic` | secret in body, no header | **Reject** | **Accepted** | **No — needs the column** |
| `post` | id+secret in body | Accept | Accept | n/a |
| `post` | Basic present | **Reject** | **Accepted** | **No — needs the column** |
| legacy | either, not both | Accept | Accept | n/a |
| any | **Basic and body both present** | **Reject** | **Accepted, Basic silently wins** | **Yes — this is purely about the request** |

So the `none` rows already hold, by the confidentiality check rather than by
a method check — the right outcome reached for a different reason, which is
worth knowing because a future change to `confidential` would break them
silently.

**Three rows do not hold.** Two need a column that does not exist. One does
not: *Basic and body both present* is a property of the request alone.

**The severity of the one that needs nothing:** RFC 6749 §2.3.1 says a client
*"MUST NOT use more than one authentication method in each request."* We
accept both and prefer Basic. I judge this **low**: an attacker who can set
the `Authorization` header already controls the request, and the body secret
being ignored does not grant anything. It is a conformance gap and an
ambiguity, not a bypass. **Say so if you disagree** — I would rather be
argued out of a low rating than have one stand because nobody pushed.

## C — the design I propose

### C1 — persist the method

Add `token_endpoint_auth_method TEXT NOT NULL DEFAULT 'client_secret_basic'`
to `clients`, surface it on `ClientRow`, and write it in the C15 insert —
which is what RFC 095 already claims happens.

**The default is the migration's real decision, not boilerplate.** Existing
rows have no recorded method. `client_secret_basic` is RFC 7591's default and
what stage 2 now defaults new registrations to, so it is the consistent
choice — **but it retroactively asserts a method for every client that
predates the column**, including administrator-created ones that may be using
`client_secret_post` today and would begin to be rejected under C2.

**This is the question I most want the review to answer.** Options as I see
them, and I am not committing to one here:

1. Default `client_secret_basic` and accept that some existing clients break.
2. Make the column nullable; `NULL` means "unrecorded", and an unrecorded
   method skips the C2 check — enforcement applies only to clients registered
   after this lands.
3. Backfill from `confidential`, which cannot distinguish basic from post and
   so is only a two-way split dressed as a three-way one.

My inclination is **(2)**, because it is the only one that cannot break a
working deployment on upgrade, and because "we never recorded this" is a true
statement that the schema should be able to express. But it leaves a
permanent class of clients outside the check, which is a real cost and may be
the wrong trade.

### C2 — enforce the method, in core

In `authenticate_client`, after the row is loaded and before the secret is
verified: if the row records a method and the presented credential's
*channel* does not match it, fail as `Unauthenticated`.

- **Same error, same timing as every other failure in that function.** It
  already runs `verify_password` against `DUMMY_PHC` on every early return
  specifically so a refusal costs the same time as a real verification. A new
  early return that skips that is a timing oracle for "this client exists but
  used the wrong channel". **Follow the existing pattern; it is there for
  this reason.**
- The channel must be passed in. `authenticate_client` currently takes
  `(db, client_id, client_secret)` and cannot tell how the secret arrived.
  Widening its signature is the straightforward route; **check whether its
  other callers** — RFC 7009 revocation and RFC 7662 introspection, per its
  own doc comment — **should enforce the same rule**, and say so. My reading
  is that they should, since the matrix says *"each row is tested for
  authorization-code and refresh dispatch"* and says nothing about exempting
  other endpoints, but those endpoints are outside RFC 095's stated scope and
  I may be over-reaching.

### C3 — reject both-channels-present

In `client_credentials`: if a Basic pair **and** body credentials are both
present, return a rejection rather than preferring Basic. Needs no column and
does not depend on C1.

**It could ship on its own if the review concludes C1 needs more thought
than this stage can carry.** Saying that now so the stage is not all-or-
nothing by default.

### C4 — the refresh half

*"Refresh is rejected unless both grant policy and scope policy authorize
it."* Every row is tested for authorization-code **and** refresh dispatch, so
the matrix is sixteen cases, not eight. Stage 2 validates `grant_types` and
the `refresh_token`/`offline_access` coupling at registration; this stage
checks them at the token endpoint, where the request actually asks for a
refresh. **Confirm the two are not already enforced somewhere I have not
found** before building anything.

## D — what I want the review to attack

1. **C1's default.** The three options, or a fourth. This is the decision
   with upgrade consequences and I have given an inclination, not a ruling.
2. **Whether C2 belongs in core's `authenticate_client` at all**, given
   revocation and introspection share it. Widening a shared authentication
   function is how a scope creeps.
3. **The severity I put on C3.** Low, argued in §B. Push if it is wrong.
4. **Whether RFC 095 needs an amendment** for the C15 paragraph that
   describes a write that does not occur.
5. **Anything in §B's table.** It is eight rows I walked by reading code; if
   a row is wrong, the design built on it is wrong.

## E — what this review is not

Not an implementation round. Nothing is dispatched and the working tree
should stay untouched. Return a review document; the stage 4 dispatch is
written against it afterwards, and will cite it the way RFC 107's did.

It does not close M3. Stages 5 and 6 follow.
