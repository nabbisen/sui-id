# RFC 152's three open questions, considered against the philosophy

> ## Whose turn it is: **the architect's.** All four are decided (§5), and recorded in RFC 152's second amendment. Q6 is
> dispatched after the `0.95.0` cut, for `0.96.0`.

**Asked.** By `@nabbisen`, 2026-11-28: *"Review and consider on them carefully with my fundamental philosophy."*
**Considered.** 2026-11-28, by the architect, at `ba7c4a6`.
**Starting from** the proposals in the cycle plan's §1
([`cycle-q-and-0.95.0-2026-11-26.md`](../149-the-console-follows-the-operator/cycle-q-and-0.95.0-2026-11-26.md)). Each was
re-measured against the code, not trusted. **One stands as written. One stands with two additions. One was wrong: its rule
broke for federated sign-in.** A fourth question, open since Q4's review, belongs with these and is added.

The two halves: (1) *"finally clean, safe and secure, and robust and sophisticated design"*; (2) *"APIs and UI/UX for users
not to be confused or misunderstand"*.

## §1: `prompt=select_account`: honour it with a two-choice page, with two additions

**The proposal stands.** The page offers *Continue as `<name>`* or *Sign in as someone else*.
- **Refusing** with `account_selection_required` turns a relying party's *switch account* button into an error page. That
  fails half 2.
- **Treating it as `login`** makes a user who wanted to stay re-enter a password. It also returns a fresh `auth_time` that
  the relying party did not ask for. That fails half 2 for the user, and blurs what `auth_time` answers.
- **The two-choice page** does what the parameter asks: the user chooses, and keeping the current account is a choice. It
  needs no new stored state.

**Two additions, both from half 2:**
1. **The page says what *Sign in as someone else* does: it signs this browser out of `<name>`, for every app that uses
   sui-id in it.** sui-id has one account per browser session (D3), so switching ends it everywhere. Without that sentence, a
   user switching accounts in one app is surprised to be signed out of another.
2. **Combinations are stated, not left to the implementer:**
   - `select_account` with no session shows the sign-in page, because there is nothing to choose between.
   - `select_account login` shows the choice, and *Continue* then asks for the credential again, for the same account.
   - `select_account none` is already `invalid_request` (D3).

**Half 1, checked.**
- *Continue* is a `POST` carrying the CSRF token. The page cannot be framed: `frame-ancestors 'none'` and
  `X-Frame-Options: DENY` (`crates/sui-id/src/http/security_headers.rs`).
- Showing `<name>` tells nothing to the relying party. The page is shown to the browser's own user, and the relying party
  receives only the code.

## §2: which sign-ins move the sign-in time: the proposal broke for federated sign-in

**The proposal.** A session's sign-in counts, and so does a completed step-up through L05 (`complete_step_up`: a TOTP code
or a passkey). B7's password re-check (`first_factor_proof`) records nothing, and does not count.

**Its own rule**: *"`auth_time` can be older than the truth but never newer."* That is the right rule for half 1 and half 2
alike, because a relying party that is told a sign-in happened when it did not cannot detect it.

**Measured: a federated sign-in breaks that rule.**
- The sign-in page (`/admin/login`) offers a *Sign in with …* button for each enabled provider
  (`crates/sui-id/src/http/handlers/admin/auth.rs:60`).
- **The request sui-id sends upstream carries no `prompt` and no `max_age`** (`crates/sui-id/src/http/handlers/federation.rs:532`).
  So the provider may answer from its own existing session, and **the user presents no credential anywhere**.
- **The provider's `auth_time` is validated and then discarded** (`crates/sui-id/src/http/optional_claims.rs`, its module
  comment: *"validated and then discarded"*).
- So with the proposal as written, `prompt=login` from a relying party would be "honoured" for a federated user by a click
  that re-authenticates no one, and `auth_time` would say it had.
  - **The relying party asked for proof of presence and was told it got it.** That is the failure D1 exists to end, moved
    one hop upstream.

**Proposed instead: the sign-in time moves only when sui-id checked a credential itself, or verified that the provider
checked one because sui-id asked.**

| Path | Counts? |
|---|---|
| A local sign-in: a password (local or directory), or a passkey, with any second factor | **Yes**, when it completes |
| A step-up through L05: a TOTP code or a passkey | **Yes** |
| A step-up with a recovery code | Already refused by step-up (RFC 102 B3) |
| B7's password re-check | **No**: it records nothing on the session. `auth_time` stays older than the truth, the safe direction |
| A federated sign-in that completes a local second factor at sui-id | **Yes**, at the second factor |
| A federated sign-in, when the request needs a fresh one (`prompt=login`, or `max_age` exceeded) | **Only if** sui-id sent `prompt=login` and `max_age=0` upstream, and the provider's ID token carries an `auth_time` no earlier than the moment sui-id sent it, within the skew the ID token check already allows |
| Any other federated sign-in | `auth_time` is the provider's `auth_time` when it sends one, never later than sui-id accepted the sign-in. When it sends none, it is the moment sui-id accepted it, **and the reference says so** (D7) |

**When a provider does not confirm a fresh sign-in**:
- the sign-in fails;
- the page says the provider did not confirm a new sign-in, and offers the user's other ways to sign in;
- nothing is issued to the relying party.

That is D1's *"honoured or refused, never ignored"*, applied one hop upstream.

**Its cost.** Two upstream parameters, one stored timestamp on the attempt row, one comparison, one message in three
languages, and a test with a mutation (the upstream `auth_time` check removed). **Proposed as stage Q6b**, directly after
Q6, so that Q6 stays the size it was planned at.

## §3: D4 with one release of warning: the answer depends on the release decision

**The proposal**: no warning release, because there is no compatibility promise before 1.0, and returning these claims
without `profile` was against the specification. That reasoning still holds. **Two things measured since make it not quite
enough for half 2:**
- **A relying party that adds `profile` to get the names back fails, until an administrator allows it.**
  - An app's non-empty scope list refuses any scope not in it: `invalid_scope`, naming the console page
    (`crates/sui-id-core/src/oidc/authorize.rs:309`).
  - So the fix takes two people: the relying party's developer, and the sui-id administrator.
  - **The upgrade note must tell both**, in that order: *"if your app shows names, request `profile`"*, and *"allow
    `profile` on that app's page"*.
- **If the fix ships as `0.95.0` and RFC 152 moves to `0.96.0`, the warning costs nothing.**
  - `0.95.0`'s notes can say: *"Planned for `0.96.0`: userinfo returns `name` and `preferred_username` only when `profile`
    was granted."*
  - That is a release of warning without delaying anything.

**Proposed**:
- if RFC 152 ships in a release after `0.95.0`, announce D4 in `0.95.0`'s notes;
- in either case, the upgrade note tells both people what to do, as above.

## §4: the fourth question: should an empty scope list keep meaning *"permit any"*?

Open since Q4's review. **It is the same question as RFC 152's, for scopes**: what a relying party asks for is honoured,
or refused.

**Measured:**
- `enforce_scope_policy` skips the check for an empty list (`crates/sui-id-core/src/oidc/authorize.rs:309`).
  - So an app with one can be issued a token carrying **any scope string it names**, defined or not.
  - A resource server that trusts the `scope` claim can then be handed a scope no administrator ever granted.
  - That is fail-open, which half 1 rules out. And *"empty means everything"* is a meaning nobody guesses, which half 2
    rules out.
- **Nothing creates one any more.**
  - Dynamic registration refuses an empty `scope` (`validate_scope`: `ScopeRejection::Empty`).
  - The app form always includes `openid` (`ensure_openid_scope`).
  - What remains is rows from before migration 0002 (v0.29.12), and lists saved empty through the old edit form,
    up to `0.94.0`.

**Proposed: end it, by migration, without breaking a working relying party.**
- Each empty list becomes an explicit one: `openid`, the standard scopes, and every scope defined when the migration runs.
- From then on, a `CHECK` refuses an empty list, so the meaning cannot return.
- **Each converted app gets an audit record**, naming the list it was given.
- **The upgrade note names the query** that finds such apps beforehand, so an operator can look first.
- **What changes for a relying party**: only a request for a scope nobody defined, which today is put into a token
  unchecked. From then on it gets `invalid_scope`, with the message that names the console page.
- **Where**: with Q6, which has a migration already, as an amendment to RFC 152 (D9). It changes what a relying party is
  refused, so it ships in the same release as the rest of RFC 152, under one upgrade note.

## §5: the decisions

**Decided 2026-11-28**, by `@nabbisen`, each as recommended: *"Two-choice page (Recommended)"*, *"Prove it upstream
(Recommended)"* and *"End it by migration (Recommended)"*. Decision 3 follows from *"0.95.0 now (Recommended)"*, which puts
RFC 152 in `0.96.0`, so `0.95.0`'s upgrade note announces D4.

1. **`prompt=select_account`**: the two-choice page, with §1's two additions (recommended). The alternatives are refusing
   it, or treating it as `login`.
2. **The sign-in time**: §2's table, with federated freshness proved upstream, as stage Q6b (recommended). The alternative
   is the original proposal, which would let `auth_time` overstate a federated sign-in.
3. **D4's warning**: announce it one release ahead if RFC 152 ships after `0.95.0`, and in either case tell both people in
   the upgrade note (recommended). The alternative is no announcement.
4. **The empty scope list**: end *"permit any"* by migration, as RFC 152's D9, with Q6 (recommended). The alternative is to
   keep it as it is, shown on the app's page since Q4.
