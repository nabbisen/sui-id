# Cycle Q and `0.95.0` — the plan

> ## Whose turn it is: **`@nabbisen`'s**, on §1's two questions.
> Q1 has landed. **Q2 can be dispatched as soon as question 1 is answered**, because every option keeps Q2 first.

**Proposed.** 2026-11-26, by the architect, after `0.94.0` was published (`b97bacb`).
**Authority already given.**
- **Cycle Q's scope** came from the split of cycle P (`@nabbisen`, 2026-11-24: *"Two releases, M2c first (Recommended)"*).
- **RFC 149's build** is under the operator-experience track's freeze exception, decided 2026-11-15.
- **RFC 152's schedule beside it** was authorized 2026-11-25 (*"Authorized."*).

This plan proposes the order, the stages and one split. **It is a proposal until it is authorized.**

## §0 — what is done, and what is carried

- **Q1 landed** (`5614f82`): the first run, tested through the binary in a real terminal. **RFC 148's last closure
  prerequisite is met**, and it moves to `done/` at `0.95.0`'s cut.
- **RFC 151** was built in `0.94.0`. It **closes when Q2 creates the *Sign-in* group**, which its closure prerequisite names.
- **Carried into RFC 149's build** (its handoff, *Carried into this RFC's build*):
  - the footer redesign (the owner's own finding);
  - the DNS deny reason's localized label;
  - the kept-backup dashboard notice;
  - `.kv-list`'s missing style;
  - the dead `page_get`;
  - C01 recording the consent policy.
- **One tidy-up**: `sui-id`'s unused `tempfile` dependency, which `cargo package` warned about at `0.94.0`.

## §1 — two questions

1. **Split cycle Q into two releases?** Recommended: **yes**.
   - **`0.95.0`**: RFC 149 (the console) and RFC 152 (what a relying party can rely on).
   - **`0.96.0`, cycle R**: RFC 101 (the email lifecycle) and RFC 150 (an administrator edits a user). RFC 150's email half
     needs RFC 101.

   **Why:**
   - RFC 149 rebuilds the chrome of every page, and RFC 101 is a security RFC of its own (311 lines: verification,
     pending changes, both addresses told, Class-A throughout).
   - In one cycle, the review of either would wait on the other, and the release would carry the risk of both.
   - Cycle P's split, for the same reason, let M2c ship on its own.
   - RFC 150 waits on RFC 101 anyway.
2. **RFC 152's three open questions**, answered in its own dispatch (Q6), with the architect's proposals then. Nothing is
   needed now. Named here so they are not forgotten:
   - `prompt=select_account`;
   - which step-up paths count as signing in again;
   - whether D4 ships with a release of warning.

## §2 — the stages of `0.95.0` (if split)

Each stage is one dispatch, one return and one review, as in cycle P. **Each RFC 149 stage closes one or more of its closure
prerequisites with a test over the rendered pages, mutated.**

| Stage | What | Closes |
|---|---|---|
| **Q1** | the first run in a real terminal | RFC 148 ✔ (landed) |
| **Q2** | **the chrome**: §1's six groups as the grouped sidebar; the account menu, with the theme selector beside Language; the footer reduced to the version; palette B's tokens (with light warning `#9D6600`); `.kv-list` styled; the dead `page_get` deleted; the *Authentication* tab shown under *Sign-in*, with every URL kept (§7) | RFC 149: *every admin page belongs to exactly one group*; RFC 151's *Sign-in* closure |
| **Q3** | **help and the dashboard**: §3's three layers on every page (a `<details>` **?**, no hover-only tooltip); §2's first-run path on the dashboard; the kept-backup notice there; the DNS deny reason's localized short labels | RFC 149: *the first-run path*; *the help policy, no hover-only tooltip* |
| **Q4** | **forms and tables**: §4's scope checkboxes and additional scopes (with the near-miss refusal); one input per redirect URI, working with JavaScript off; create and edit as one form; C01's consent-policy attribute; §5's counts, pages and the audit log's sticky header | RFC 149: *§4's editors work with JavaScript off* |
| **Q5** | **RFC 152, the request**: `max_age` and `auth_time` (a migration: the code and the refresh token carry the sign-in time), `prompt` honoured or refused, and the other requirement-type parameters refused | RFC 152 D1–D3 |
| **Q6** | **RFC 152, the rest**: userinfo's `profile` scope (D4); a signing key published before it signs (D5); discovery's claims (D6); the reference's parameter table (D7) and D8's relying-party obligations | RFC 152 D4–D8 |
| **—** | the `tempfile` tidy-up, folded into whichever stage touches `crates/sui-id/Cargo.toml` first | — |

**Order and parallelism.**
- Q2 first, because Q3 and Q4 are built inside its chrome.
- **Q5 and Q6 touch no console page**, so they can run beside Q3 and Q4, one at a time with the dev team, alternating while
  the architect reviews.
- The release preparation follows the last of them.

**Closure.** At `0.95.0`'s cut:
- RFC 148 to `done/`;
- RFC 151 to `done/`, once Q2 has landed;
- RFC 149 to `done/` once its closure prerequisites are all met (Q2–Q4);
- RFC 152 to `done/` once Q5 and Q6 have landed.

## §3 — what this plan does not include

- **RFC 009** (other databases) and **the embedding work**. Both are frozen for building; their designs stay under review.
- **RFC 097 stage 2** (the STRIDE worksheets). It is the architect's measurement work, and can run beside any stage without a
  release. It will be proposed separately.
- **No dates.** A failed gate moves the release, and scope is never cut to keep one.
