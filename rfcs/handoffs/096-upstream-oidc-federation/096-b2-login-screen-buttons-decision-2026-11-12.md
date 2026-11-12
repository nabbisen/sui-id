# RFC 096-B2 — the login screen has no federation buttons, and one question for S6

> ## Accepted 2026-11-12. `@nabbisen`: *"Both your recommendations are accepted."*
> **§3:** the login-page buttons are built in cycle L, as **S5b**, which is dispatched —
> [`096-b2-stage5b-the-login-page-buttons-2026-11-12.md`](096-b2-stage5b-the-login-page-buttons-2026-11-12.md).
> **§4:** C18 also refuses a provider a configuration block still names. That
> is recorded in RFC 096's A3, at `:170`, and S6 builds it.

**Prepared by.** The architect, 2026-11-12, while reviewing S5.

## 1 — what was found

**sui-id's login page has never offered federated sign-in.** No version of
it rendered a "Sign in with …" button. The only way to start federated sign-in
is to type `/auth/federated/{slug}/start` into the browser.

- **RFC 004, `Implemented (v0.76.4)`, designs the buttons.**
  `rfcs/done/004-federation.md:72`: *"`sui-id-web` — login-screen provider
  buttons (driven by enabled …)"*. And `:155`: *"The login screen lists
  enabled providers as buttons."* They were never built.
- **`list_enabled`**, the repository function documented *"for the login
  screen"* (`repos/federation_provider.rs:76`), has no production
  caller.
- **The documentation promises the buttons in four places:**
  - `docs/src/reference/oidc-api.md:195`;
  - `docs/src/guides/operators.md:1686`;
  - `docs/src/reference/configuration.md:246, :251`, where `display_name` is
    *"Human-readable label for the 'Sign in with X' button"*;
  - `docs/src/getting-started/overview.md:79`.
- **So `display_name` is shown nowhere an end user sees it**, and C26, built
  in S2b, updates a label no one can read.
- **Cycle L's second protection holds only by accident.** *"The login screen
  does not offer a disabled provider at all"* (`cycle-l-and-0.90.0-2026-11-11.md:129`)
  is true because it offers no provider at all.

**My own record said otherwise.** The cycle L scope (`:73`) and the stage
plan's §1 both state that the login screen lists providers. I cited
`list_enabled` without tracing its callers. Both statements are now
corrected in place.

## 2 — why it matters now

0.90.0 is the release in which an administrator first re-enables federated
providers, through S5's page. Without the buttons, that release ships a
federation feature its users can reach only by a URL they would have to be
given. And the documentation says otherwise in four places. RFC 096's own
end-to-end promise, a user who signs in through an upstream, depends on a
step that does not exist.

## 3 — your decision

**Recommended: build the buttons in cycle L, as stage S5b, before S6.**
- **The design is RFC 004's**, already accepted and implemented in every
  other part. This completes what shipped; it designs nothing new.
- **What it is.** The login page lists every enabled provider (through
  `list_enabled`, ordered by `display_name`) as a "Sign in with
  {display_name}" link to `/auth/federated/{slug}/start`. It carries the
  page's own `next`, so an OIDC authorization that sent the user to log in
  resumes afterwards, as the password path does. With no enabled provider,
  nothing is shown.
- **What it needs.** The label is translated in Japanese and English. It
  respects the login page's existing context (RFC 091). The button is a plain
  link, so nothing new runs on the server.
- **Tests.**
  - an enabled provider appears;
  - a disabled one does not, which makes cycle L's second protection real;
  - `next` survives;
  - `display_name` is escaped;
  - **a sign-in started from the button completes.**

**The alternative** is to leave the login page as it is, and correct all
four documents to say federated sign-in starts at a URL the operator gives
its users. RFC 004's gap would be recorded, not closed. I don't recommend
it: an identity provider that documents a sign-in button and has none fails
the "users cannot misunderstand" half of your philosophy on the very page
users see first.

Either way, the four documents are corrected in the same stage.

## 4 — a second question, for S6: may a still-configured provider be deleted?

Amendment A3, as you approved it (RFC 096 `:167-170`), allows C18 delete
*"only for a provider already disabled"*. **Measuring for S6 found a case
that rule lets through and an operator would misread.**

A provider can be disabled while its `[[federation_providers]]` block is
still in the config file, for example after an administrator's C17
disable. If it is deleted then:
- its links cascade away, and the confirmation says so;
- **on the next restart, startup's C16 sees the configuration block, finds no
  row, and creates the provider again**, with a new id and no links.

The administrator sees the provider they deleted come back, disabled and
requiring review, with every user's link gone. Nothing warned them the
delete would undo itself.

**Recommended:** C18 also requires that **no configuration block names the
provider**. A refusal then says *"remove its [[federation_providers]] block
and restart sui-id first"*. In practice, the only deletable provider is one
C25 already disabled because its block was removed. The delete then means
what an administrator expects: the provider is gone for good.

This adds a precondition to an accepted amendment, so it is yours. **The
alternative** is A3 as written, with the confirmation page warning that a
provider whose block remains will be recreated on restart.
