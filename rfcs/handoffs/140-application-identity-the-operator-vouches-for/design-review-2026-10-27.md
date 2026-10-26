# RFC 140 — design review

**Reviewer.** The architect (high-capability model), **which also wrote
this RFC** — so this review is **not independent of it**. Filed as the
review RFC 140's `Approved by` field refers to.

**`ROADMAP.md` §S1c prohibits architect self-review by default and
permits it where `@nabbisen` asks, requiring the record to say he asked.
He asked, 2026-10-27, in his own words:** *"Over-engineering ? Worth
scoping it or not ? Review it with my fundamental philosophy."* This file
is the record of that.

**Scope.** Read-only. No tracked file changed by the review itself.
**Measured** 2026-10-27 against `e9065ee`.

## 1 — the question

Whether to build an administrator-side write path for the four
application-identity fields, or whether doing so is over-engineering.

## 2 — what was measured

**The write side.** `POST /oauth2/register` sets all four
(`http/handlers/dynamic_register.rs:503` for `client_uri`). The
administrator path hardcodes them — `identity/admin/clients.rs:96`,
`homepage_uri: None` and its neighbours. `repos::clients::update_app_identity`
exists and has **no caller anywhere in `crates/`**.

**The read side.** `sui-id-web/src/pages/oidc.rs:70-73` renders
`logo_uri` only for `RegistrationSource::Admin`; RFC 095 M3 stage 5 added
that gate so a registrant-controlled logo origin is never fetched.

**So the logo is dead in both directions**: the branch that may render
can never hold a value, and the branch that holds values never renders.
A migration (`0035`), an `<img>` tag, CSS, a caption string and a
security gate, for a path that cannot display anything.

**And the CSP blocks it anyway.** `http/security_headers.rs:70` sets
`img-src 'self' data:`, asserted by
`tests/e2e/r095_m3_stage5_browser_presentation.rs:118`. **Even an
administrator-set `logo_uri` would not load.** This is the finding that
changed the recommendation: the architect's first cost estimate to
`@nabbisen` — *"four fields, validation already written"* — is true of
three of the four and false of the logo. Correcting it before he
committed effort is the main thing this review did.

**One consistency fix is being made separately**, in `0.85.0`'s M2b wave
4 stage 4: the consent screen's provenance gate is extended to cover
`homepage_uri` as it already covers `logo_uri`. It is independent of this
RFC and lands whichever way RFC 140 goes. Its own analysis is held out of
this repository under `.github/SECURITY.md`'s disclosure rule until
`0.85.0` ships.

## 3 — against the stated philosophy

`@nabbisen` asked for both halves, so both are applied.

**"Clean, safe and secure, robust and sophisticated."** The present state
is worse than the feature being absent — absent would be clean. What
exists is a half-built feature whose trust gate was written for a path
that has no data, and whose trusted write path was never built. A reader
of `oidc.rs` would conclude administrator-set application identity is
supported. It is not.

**"APIs and UI/UX for users not to be confused or misunderstand."** The
consent screen is the one page in an identity provider where an **end
user** — not an operator — makes a trust decision. A logo slot that can
never fill, beside a client name whose link comes from somewhere the page
does not explain, is the second half failing on the page where it matters
most.

## 4 — conclusion

**Not over-engineering, and not urgent.** The schema, the validation
function and the provenance gate are already paid for; what is missing is
the half that makes them mean something. Scoped as RFC 140 with the logo
split into its own stage, because §5 of that RFC is a real decision and
not a form field.

**The alternative is recorded rather than dismissed**: if the RFC is
declined, deleting the dead display code is better than leaving it. That
remains true and is written into RFC 140 §6.
