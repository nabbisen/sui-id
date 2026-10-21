# M3 stage 4a — persist the three fields

> ## Whose turn it is: **the implementation role's.**
> Nothing in this stage waits on `@nabbisen`. The three decisions stage 4
> turned on are settled — §1 — and none is reopenable here.

**Dispatched.** 2026-10-22 JST, by the architect.
**Design.** [`stage4-design-the-authentication-grant-matrix-2026-10-21.md`](stage4-design-the-authentication-grant-matrix-2026-10-21.md)
— §A, §C1, §D and §G. Read §A and §C1 before starting.
**Normative source.** [`metadata-validation.md`](metadata-validation.md)
§*Supported fields*, and RFC 095 `:362-366` for what the C15 insert writes.
**Stage 6's review.** `.git-exclude/reviewed/rfc-095-stage6-legacy-candidates-2026-10-21.md`
**Baseline.** Read the tip with `git log -1`.

## 0 — why stage 4 is two dispatches

Stage 4 as designed spans a migration, three new columns, an HTTP-layer
type change, client authentication logic and a timing-uniformity fix. That
is four kinds of verification in one round, on **the one stage where a
wrong accept is an authentication bypass rather than a validation gap** —
precisely the shape [`m3-stage-plan-2026-10-20.md`](m3-stage-plan-2026-10-20.md)
§A says produces rounds where defects hide.

- **4a, this one: persistence only.** The three fields get stored and read
  back. **No behaviour changes at `/oauth2/token`.**
- **4b, after it: enforcement.** The typed presentation, the channel check,
  RFC 123 D4 compliance, both-channels-present, and the refresh half.

4b needs 4a's stored method and cannot start without it. Splitting costs
one extra round and buys a round whose verification is one thing:
does the value written come back.

## 1 — the decisions, settled; do not reopen them

| | Decision |
|---|---|
| **C1** | A **nullable** column. `NULL` means *unrecorded*, is exempt from 4b's check, and **the exempt class must be visible to the operator.** |
| **C2** | 4b adds the channel check *and* brings `authorize::authenticate_client` into RFC 123 D4 compliance in one change. |
| **C3** | Reject both channels present; may ship independently of C1. |

C3 was settled by `@nabbisen` on 2026-10-22; C1 and C2 by the architect at
his request, recorded in the design's §D under `ROADMAP.md` §S1c.

**`client_secret_basic` as the migration default is refused**, and §C1 says
why at length: it asserts a fact never recorded, it is the inference RFC
095 `:352-353` forbids by name, and it would silently start refusing an
administrator-created client that presents its secret in the body. **If you
find yourself wanting to backfill a value, stop and return the question.**

## 2 — the migration

**`0049`** is next (`0048_federation_mfa_pending_one_active_per_user.sql` is
the highest today — confirm before you write it; a competing migration
number is a stop condition in this package's README).

Three columns on `clients`, all nullable, no default:

- `token_endpoint_auth_method TEXT`
- `grant_types TEXT`
- `response_types TEXT`

**Nullable with no `DEFAULT`, deliberately.** A `DEFAULT` would make every
pre-existing row claim a value nobody recorded, which is the thing §C1
refuses. `NULL` must be distinguishable from "recorded as the default",
because 4b exempts the first and checks the second.

**The two list columns:** `grant_types` and `response_types` are stored as
JSON arrays, matching `redirect_uris` and `post_logout_redirect_uris` on
the same table. Use the existing `require_valid_json::<Vec<String>>`
pre-condition in `repos/clients.rs` rather than a new one — it is there so
a future read cannot meet corrupt JSON, and these columns earn it equally.

**Report what the migration left unrecorded.** It must be able to say how
many rows it did not fill. That number is the size of the class 4b will not
check, and an operator upgrading deserves it at upgrade time, not by
querying later.

## 3 — the model and the write path

`ClientRow` (`sui-id-store/src/models.rs:294-320`) gains three
`Option<...>` fields. Typed where a type exists: stage 2 already has
`validate_auth_method` (`dynamic_registration_validation.rs:701`) returning
a `&'static str` from a closed set of three. **Do not widen that to a bare
`String` in the model if a small enum serves** — decide, and say which you
chose and why. `grant_types`/`response_types` are `Option<Vec<String>>`.

`clients::create_within_tx` writes all three **from the row**, in its own
INSERT, exactly as it already writes `consent_policy` and the four
application-identity URIs since 2026-10-02.

> **Do not touch `registered_via` while you are in that INSERT.**
> `create_leaves_registered_via_at_the_table_default` exists to stop that
> and its doc comment explains why; the architect proposed removing it on
> 2026-10-21 and withdrew. Three new columns go in; that one stays out.

**Every `ClientRow` construction site must be updated**, and there are more
than the compiler will make obvious in a pleasant order:
`repos/clients.rs`, `core/identity/admin/clients.rs`,
`http/handlers/dynamic_register.rs`, and the test fixtures in
`repos/refresh_tokens/tests.rs`, `repos/auth_codes/tests.rs`,
`repos/audit/tests_rfc095_stage6.rs`, `tests_state_machine/auth_codes.rs`,
`tests_state_machine/refresh_tokens.rs`, `commands/tests/runner.rs`,
`core/oidc/authorize/redirect_uri_matches_tests.rs` and `tests/e2e/r123.rs`.
**An admin-created client gets `None` for all three**, not a default —
nobody asked it for a method either.

## 4 — the registration path stores what it already validates

`handlers/dynamic_register.rs` validates all three today and echoes them;
it stores none. Wire the validated values into the `ClientRow` it builds.

**The echo must not change.** Stage 3's e2e asserts the response shape, and
a dynamic registration that omits these members still gets stage 2's
defaults echoed back. What changes is only that the echoed value is now
also the stored one. **Prove that**: read the row back from the database
and compare it to the response body, in the same test. Stage 3 did this for
the fields that had storage; these three are the ones it could not.

## 5 — the operator-facing half of C1, which is not optional

**A client whose method is unrecorded must read as unrecorded.** Not blank,
not rendered as `client_secret_basic`, not absent from the row.

`sui-id-web/src/pages/clients.rs` already carries the precedent: RFC 136 D2
puts a marker on `registered_via = 'dynamic'` and only there (`:34-37`,
`:280`). Follow that shape for an unrecorded auth method. Keep it to the
client views that already show provenance; this is not a new screen.

**Why this is in the dispatch rather than left to taste.** 4b will not
check these rows. An exemption nobody can see is indistinguishable from an
enforcement gap, and the operator is the only person who can act on it. The
exit path — an administrator action that records a method on an existing
client — is **§G item 1 and is not built here.**

## 6 — tests

**Storage round-trip, per field.** Write a non-default value, read it back,
assert it survived. Then the one that matters: **write `None` and assert it
reads back `None`**, not a default — that is the whole C1 decision as an
executable statement, and it is the assertion a future `DEFAULT` would
break.

**An upgrade test.** A database created before `0049`, migrated, must have
`NULL` in all three columns on every pre-existing row and must still serve
`/oauth2/token` exactly as before. 4a changes no authentication behaviour
and the test should say so by exercising it.

**The echo-equals-storage test** from §4.

**Visibility**, per §5, asserted on the rendered output as stage 5 did —
`sui-id-web` renders to `String`, so assert on it directly rather than
through a browser.

**Mutation evidence, two.** (a) Give the migration a `DEFAULT
'client_secret_basic'` and show the `None` round-trip test failing. (b) Drop
one of the three columns from `create_within_tx`'s INSERT and show the
corresponding round-trip failing. This project has twice shipped a test
that passed while testing nothing; both of those were caught by exactly
this.

## 7 — what to return

The usual package, plus:

- **Whether you typed the auth method as an enum or a `String`**, and why.
- **The migration's unrecorded-row count** — where it surfaces and what it
  says.
- **Confirmation that `0049` was free** when you took it.
- **The full list of `ClientRow` sites you touched**, so the next round can
  tell whether anything was missed rather than discovering it.
- Both mutations from §6.
- The full matrix; Level B scope. Report it local.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the last
  commit of the round, baselined on the tip at that moment.

**`CHANGELOG`: yes, `### Added`** — three stored fields and a migration are
integrator- and operator-visible. **Say explicitly that no authentication
behaviour changes in this release**, because a reader who sees
`token_endpoint_auth_method` appear will reasonably assume it is now
enforced. It is not, until 4b.

## 8 — what this stage does not do

No channel enforcement, no `DUMMY_PHC` work, no both-channels-present
rejection, no refresh-half grant checking — all 4b. No administrator action
for recording a method on an existing client (§G item 1). No change to
`registered_via`, to the C15 transaction's shape, or to the introspection
and revocation path.

**It does not close M3**, and `0.83.0` does not cut until 4b lands too.

Leave the working tree uncommitted. One open dispatch at a time; this is it.
