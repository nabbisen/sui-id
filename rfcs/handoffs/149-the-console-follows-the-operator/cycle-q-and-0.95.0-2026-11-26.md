# Cycles Q and R: `0.95.0` and `0.96.0`, the plan

> ## Whose turn it is: **the implementation role's, on `0.95.0`'s release preparation.**
> Q2 (`57c80d1`), Q3 (`420001b`), Q3b (`18615bd`), Q4 (`14a0422`) and Q5 (`38f99f0`) have landed. **RFC 149's four closure
> conditions are met.**
> **Re-scoped 2026-11-28** (`@nabbisen`: *"0.95.0 now (Recommended)"*): `0.95.0` is cut after Q5, with the audit CSV fix
> (CHANGELOG, *Security*). Q6 to Q8 ship as `0.96.0`, and cycle R as `0.97.0`. The version numbers below are the plan's
> original ones; where they differ, this note governs.
> RFC 152's three questions, and a fourth, were settled on 2026-11-28
> ([considered](../152-what-a-relying-party-can-rely-on/open-questions-considered-2026-11-28.md)). One question remains, before R3 (§1).

**Authorized 2026-11-26.** `@nabbisen`, on the split proposed in this plan's first version (`6577468`): *"Yes. Scope them, the
series, carefully with consideration."* This version is that scoping, from measurements made the same day at `6577468`.
**The scoping was authorized the same day**, as published at `f0107e1` (`@nabbisen`: *"Authorized."*). That covers the
stages of §2 and §3, and the choices made in Q2's dispatch. §1's four answers remain proposals. Each is put to him again
before its stage.

**Authority already given.**
- **Cycle Q's scope** came from the split of cycle P (`@nabbisen`, 2026-11-24: *"Two releases, M2c first (Recommended)"*).
- **RFC 149, RFC 101 and RFC 150** are built under the operator-experience track's freeze exception, decided 2026-11-15.
- **RFC 152's schedule beside RFC 149** was authorized 2026-11-25 (*"Authorized."*).

**What re-scoping found.** The first version was written from the records. Measuring the code changed four things:
1. **Palette B has already shipped.** RFC 144 built it in `0.92.0` (`crates/sui-id-web/src/tokens.rs:7`, with
   `--warning-default: #9D6600` at `:76`). The first version listed it as Q2 work. It is not.
2. **RFC 152's D5 touches a console page.** The signing-keys page must say when a new key starts signing. So it follows Q2,
   like the console stages. The first version said that Q5 and Q6 touch no console page.
3. **The first version's Q4 was two stages.** One rebuilds the app form; that holds RFC 149's fourth closure condition and
   changes an audit event. The other touches every list page. Each is now its own stage.
4. **D5 is its own stage.** A stage that chooses which key signs every token is not one to share with documentation.

## §0: what is done, and what is carried

- **Q1 landed** (`5614f82`): the first run is tested through the binary, in a real terminal. **CI confirmed it on
  `ubuntu-24.04`** (run `45806896282`, 29 of 29 jobs). All five terminal tests passed there. **RFC 148's last closure
  prerequisite is met**, and it moves to `done/` at `0.95.0`'s cut.
- **Q2 landed** (`57c80d1`): the grouped sidebar, the account menu, and the footer reduced to the version inside the console.
  Its review folded in what a browser walk found at 200% font, and put each page's item into `Chrome::Console`'s type.
- **Q3 landed** (`420001b`): help in three layers, the first-run path, the dashboard's attention items, the DNS labels, and
  one language chain. It closes RFC 149's second and third conditions. Its review corrected four help texts against the code,
  and found two things outside RFC 149, decided the same day (§1).
- **Q3b landed** (`18615bd`): the password-breach check can be changed on Sign-in settings. Lowering it is a dangerous
  action, and the direction is decided inside the command's transaction. Dev mode stores `off` when it seeds, as its texts
  always said.
- **Q4 landed** (`14a0422`): the app form. It closes RFC 149's fourth condition, so all four are met. Its review named, on
  the edit page, an app whose stored scope list is empty, since an empty list permits any scope, and kept the
  JavaScript-free row buttons working before the name is filled. **Whether an empty list should keep meaning "permit any"
  is a question for `@nabbisen`**, kept for compatibility since v0.29.12, and not changed here.
- **`main` was red on Rust 1.95 between `099da5c` and `2d947cd`**, from Q2's review fold-in. An incremental 1.95 build had
  passed it. Every console stage now builds `sui-id-web` from clean on 1.95 before its matrix.
- **RFC 151** was built in `0.94.0`. **Its last condition is met by Q2**: the minimum is raised *"from the *Sign-in*
  settings"*, asserted by `q2_navigation::rfc151_closure_the_minimum_length_is_under_sign_in`. It moves to `done/` at
  `0.95.0`'s cut.
- **Carried into RFC 149's build**, from its handoff's *Carried into this RFC's build*, each placed in a stage below:

  | Carried item | Stage |
  |---|---|
  | the footer redesign (the owner's own finding) | Q2 |
  | `.kv-list` has no style | Q2 |
  | the dead `/me` page (`page_get`, now in `handlers/me_security/overview.rs`) | Q2 |
  | the kept-backup dashboard notice | Q3 |
  | a short localized label for each DNS deny reason | Q3 |
  | C01 records the consent policy | Q4 |

- **One tidy-up**: `sui-id`'s unused `tempfile` dependency, which `cargo package` warned about at `0.94.0`. It is done in Q2.

## §1: decisions still to come, with the architect's proposals

**Decided 2026-11-27 for Q3**, by `@nabbisen`, on the architect's questions during Q2's review:
- the first-run path's fourth step: *"Optional, no done state (Recommended)"*. That replaces the acknowledgement proposed
  in §2 below;
- the language: *"One chain everywhere (Recommended)"*. Every page resolves the language as the person's preference, then
  the cookie, then `Accept-Language`, then the server default. Measured in Q2's review: the console used a chain of its own,
  without the cookie and `Accept-Language`.

None of these blocks Q2 to Q5. Each is settled before the stage named.

**Decided 2026-11-27 on Q3's review**, by `@nabbisen`:
- **the password-breach (HIBP) mode can be changed in the console**: *"Small stage in 0.95.0 (Recommended)"*. That is
  **Q3b**, on Sign-in settings. Weakening it is a dangerous action (RFC 058): confirmation, step-up, audit note;
- **`backup create` with the key from `SUI_ID_MASTER_KEY`**: *"Include it only encrypted (Recommended)"*. The key is
  included only with `--encrypt`; without it, the command refuses and says why. It never writes an environment secret to
  disk in plain text. **Scheduled in cycle R** (§3).

**Before Q6** (RFC 152's three open questions):

1. **`prompt=select_account`.** **Proposed: honour it with a two-choice page.** The page offers *Continue as `<name>`* or
   *Sign in as someone else*.
   - **Why:** that is what the parameter asks for: the user chooses an account, and keeping the current one is a choice.
   - **Refusing it** with `account_selection_required` turns a relying party's *switch account* button into an error.
   - **Treating it as `login`** makes a user who wanted to stay re-enter their password.
   - **The cost**: one page on the sign-in surface, with no new stored state. *Continue* resumes the request without a new
     sign-in. *Someone else* is `prompt=login`'s path.
2. **Which step-up paths count as presenting a credential** for D2's sign-in time. **Proposed: a session's sign-in, and a
   completed step-up through L05** (`complete_step_up`, `crates/sui-id-store/src/commands.rs`: a TOTP code or a passkey).
   - L05 records `last_step_up_at`.
   - **B7's password re-check does not count.** B7 is `first_factor_proof`, the password asked before a sensitive action
     (`crates/sui-id-core/src/authn/step_up.rs`). It records nothing on the session.
   - **Why:** with this rule, `auth_time` can be older than the truth but never newer. A relying party that relies on it may
     ask for a sign-in that was not strictly needed. It is never told that a sign-in happened when it did not.
3. **Should D4 ship with one release of warning?** **Proposed: no**, as RFC 152 already argues. There is no compatibility
   promise before 1.0, and returning these claims without `profile` was against the specification. The change is named in the
   CHANGELOG's *Changed*, and in the upgrade note, for anyone who reads `name` without asking for `profile`.

**Before R3** (RFC 101's §6.2, which it leaves open):

4. **The event names and their notes.** The proposed names are `user.email_change_requested`, `user.email_changed` and
   `auth.email.verified`. RFC 101 asks for *"a second reader against the namespace split"* recorded in
   `audit-coverage-matrix.md`. The architect will measure the names against that split and propose the notes in R3's
   dispatch, for `@nabbisen` to settle there.

## §2: `0.95.0`, cycle Q

One dispatch, one return and one review per stage. **Each RFC 149 stage asserts its closure conditions with a test over the
rendered pages, and each test is mutated.**

| Stage | What | Closes |
|---|---|---|
| **Q1** | the first run in a real terminal | RFC 148 ✔ (landed; CI ✔) |
| **Q2** | **the chrome.** The grouped sidebar, generated from one list. The account menu gains Language and the theme selector. The footer goes; the version stays, shown only inside the console. The settings tabs give way to the sidebar's items. `.kv-list` is styled; the dead `/me` page is deleted; `tempfile` is removed. Every URL is kept | RFC 149: *every admin page belongs to exactly one group*. RFC 151's *Sign-in* condition |
| **Q3** | **help and the dashboard.** §3's three layers on every console page: a purpose line, `<details>` **?** with *Learn more*, and no `title=` tooltip anywhere. §2's first-run path, and a *needs attention* section for an overdue kept backup and for a federation provider that requires review. The DNS deny reasons' localized labels | RFC 149: *the first-run path*; *the help policy, no hover-only tooltip* |
| **Q3b** | **the password-breach check can be changed**, on Sign-in settings; weakening it is a dangerous action | — (a finding of Q3's review) |
| **Q4** | **the app form.** §4's scope checkboxes; additional scopes, validated, with near misses refused; one input per redirect URI and per post-logout URI, working with JavaScript off; create and edit as one form; C01 records the consent policy | RFC 149: *§4's editors work with JavaScript off* |
| **Q5** | **the lists.** §5: every list states its count and its page, and none truncates silently; the audit log keeps its five columns and its header in view | RFC 149 §5 (not a closure condition, but in its scope) |
| **Q6** | **RFC 152, the request.** D1: every parameter that states a requirement is honoured or refused. D2: `max_age`, and `auth_time` in every ID token, with a migration so that the code and the refresh token carry it. D3: `prompt`, with §1's answer for `select_account` | RFC 152 D1–D3 |
| **Q7** | **RFC 152, the key.** D5: a new signing key is published before it signs. The signer is chosen per token, with one constant shared with the JWKS `Cache-Control`. The signing-keys page says when the new key will start signing | RFC 152 D5 |
| **Q8** | **RFC 152, claims and the reference.** D4, userinfo's `profile` scope. D6, discovery's claims, generated from the serialized list. D7, the reference's parameter table. D8, the relying party's obligations, each row checked against the code | RFC 152 D4, D6–D8 |
| **prep** | the release preparation (the mechanical half, as for `0.94.0`) | — |

**What each stage must settle that the RFCs leave to the build.** These are named now so that no dispatch meets them by
surprise:
- **Q3, the first-run path's fourth step.** *"Review sign-in and session settings"* has no state from which "done" can be
  read. **Settled 2026-11-27** (§1): it is *Recommended*, with no done state, and like step 5 it never holds the path open.
  **A step must never show as done because of something that does not mean it was done.**
- **Q4, apps saved before `0.95.0`.** An app whose stored scopes include a near miss (`profle`) cannot be saved again until
  it is corrected. The page names the scope and offers the standard one. The scope was never in effect, so nothing that
  worked stops working. The upgrade note says so.
- **Q6, `prompt=login` and `max_age`.** The new sign-in creates a new session and ends the old one in one transaction,
  whoever signs in. One browser never holds two sessions, and a different user signing in is an account switch, not a
  merge.
- **Q7, a retired key's deletion.** The help says to wait the longer of the two token lifetimes (Q3's review). The page
  offers *Delete* at once. Q7 rebuilds this page for D5, and says per key whether tokens it signed can still be valid.
- **Q7, a fresh installation.** No key has been published for 300 seconds, so the rule *"the newest key published at least
  that long ago"* chooses none. The rule is completed so that the oldest published key signs. A key that is signing, or due
  to sign next, cannot be deleted.

**Order.**
- Q2 comes first, because every later stage renders inside its chrome.
- Then Q3, Q4 and Q5, in that order. Q3's help layer is used by Q4's additional-scopes field.
- **Q6, Q7 and Q8 can alternate with Q3 to Q5**, one stage with the dev team at a time, while the architect reviews the
  other. Q8 comes last of the three, because its reference table documents what Q6 and Q7 built.
- The release preparation follows the last stage.

**Closure at `0.95.0`'s cut.**
- RFC 148 moves to `done/`.
- RFC 151 moves to `done/` once Q2 has landed.
- RFC 149 moves once Q2, Q3 and Q4 have landed (its four conditions).
- RFC 152 moves once Q6, Q7 and Q8 have landed.

## §3: `0.96.0`, cycle R: RFC 150 and RFC 101

This cycle edits a user and gives an email address its lifecycle.

**Prerequisites, measured:**
- RFC 101 requires RFC 103 to be implemented (it is in `done/`), and the Class-A seam, which closed in `0.88.0`.
- RFC 150's email half requires RFC 101.
- **Both are built inside RFC 149's console.** RFC 150 §4: *"The page's layout follows RFC 149, if that has been built by
  then."* It will have been.

| Stage | What | Closes |
|---|---|---|
| **R0** | **`backup create` with the key from `SUI_ID_MASTER_KEY`**: included only with `--encrypt`; otherwise a refusal that says so. Decided 2026-11-27 (§1) | — (a finding of Q3's review) |
| **R1** | **RFC 150 §1 and §1a.** The display name and the language, one Class-A command each, edited in one form on the user's own page. An unchanged field writes nothing. A directory user's name and email are read-only, with RFC 150's one line, and the command refuses them. The test that fails on any production call to `users::update_email` | RFC 150: *name and language*; *directory user read-only*; *no direct email write* |
| **R2** | **RFC 150 §2 and §5.** A rename, with step-up, recording the old and the new name. The table of retired names (a migration), checked when a user is created or renamed. `admin.username.released`, with step-up and its confirmation page, for one name at a time. The test that fails if any other path frees a name | RFC 150: *rename*; *a retired name is released only deliberately* |
| **R3** | **RFC 101 D1, D5, D6 and §6.4/§6.5: verifying the current address.** A single-use, hashed, expiring token, sent to the address itself. Consuming it sets `email_verified_at` (Class-A, audited). The OIDC `email_verified` claim follows it. Signed-in users are prompted, never blocked. **From this stage, password reset sends a link only to a verified address**, with no transition period. Requests are rate-limited as forgot-password is. The command inventory gains RFC 101's rows | RFC 101: *a user can verify an address*; *`email_verified` reflects reality* |
| **R4** | **RFC 101 D2–D4 and D7: a user changes their address.** A pending new address. A confirmation screen, step-up, and an audit note (RFC 058's three obligations). Confirmation goes to the new address and a notice to the old one. The change revokes outstanding reset tokens, and a competing change voids the earlier token. The response never reveals that an address is in use | RFC 101: *change*; *recovery cannot be redirected by session compromise alone* |
| **R5** | **RFC 101 §6.3 and RFC 150 §3: an administrator starts a change** from the user's page, through R4's lifecycle. The address an administrator sets is unverified until the user proves it. Never for a directory user | RFC 150: *email through RFC 101* |
| **prep** | the release preparation | — |

**Why this order.**
- R1 and R2 change only who may write what, and build the user's edit form that R5 extends.
- R3 lays the token and mail path that R4 and R5 reuse.
- **R3 changes password reset for every existing user.** On the day it ships, no address is verified, so no one can reset a
  password by email until they verify. The ruling of 2026-09-16 chose that, with no transition period, because an
  administrator's recovery link (RFC 103) means nobody is stranded. The upgrade note must say it first. RFC 152 D8's R5 row
  (*"every user, until RFC 101 ships"*) is updated in the same stage.

**Closure at `0.96.0`'s cut.** RFC 150 and RFC 101 move to `done/` once all their conditions are met.

## §4: what this plan does not include

- **RFC 009** (other databases) and **the embedding work**. Both are frozen for building, and their designs stay under
  review.
- **RFC 097 stage 2** (the STRIDE worksheets). It is the architect's measurement work, which can run beside any stage without
  a release. It will be proposed separately.
- **Adding a federation provider from the console.** It stays after M7.
- **No dates.** A failed gate moves a release, and scope is never cut to keep one.
