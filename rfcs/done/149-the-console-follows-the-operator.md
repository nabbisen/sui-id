# RFC 149 — The admin console is organised around what an operator does

**Status.** Implemented (v0.95.0)
**Closure reviewed on.** 2026-11-28, on the architect's [closure assessment](../handoffs/149-the-console-follows-the-operator/closure-assessment-0.95.0-2026-11-28.md) of RFCs 148, 149 and 151.
**Closure approved by.** `@nabbisen`, 2026-11-26, authorizing the cycle Q plan (`f0107e1`), which closes it at `0.95.0`'s cut: *"Authorized."*
**Closure evidence.** The [closure assessment](../handoffs/149-the-console-follows-the-operator/closure-assessment-0.95.0-2026-11-28.md), condition by condition: `e2e::q2_navigation::every_admin_get_route_belongs_to_exactly_one_group` (Q2); `e2e::q3_help_and_dashboard::the_first_run_path_opens_with_recommended_and_optional_steps_and_folds`, `every_console_and_account_page_has_a_nonempty_lede` and `no_hover_only_tooltip_anywhere` (Q3); the `e2e::q4_app_form` tests, which post plain forms (Q4). Each was mutated and caught.
**Accepted on.** 2026-11-15
**Approved by.** `@nabbisen`, 2026-11-15: *"All accepted and authorized."*, on the architect's review of its mockup questions.
**Security review.** Required
**Design prerequisites.** RFC 144 Accepted. Its five rules are the floor any
palette here must meet.
**Implementation prerequisites.** This RFC Accepted, **once its mockups
(handoff, `mockups/`) have been reviewed**, with §8's questions answered.
Under the operator-experience track, nothing is built until the track's
schedule and every one of its RFCs exist (ROADMAP, *Active plan*,
2026-11-15).
**Closure prerequisites.** All of the following:
- every admin page belongs to exactly one navigation group of §1;
- the first-run path of §2 is reachable from the dashboard;
- §3's help policy is applied to every page, and no page uses a hover-only
  tooltip;
- §4's list editors and scope selection work with JavaScript off;
- each is asserted by a test over the rendered pages, and each test is
  mutated to show it can fail.
**Tracks.** The operator-experience track, Track C (structure): the admin
console.
**Touches.** `crates/sui-id-web/src/layout.rs`, `crates/sui-id-web/src/pages/**`,
`crates/sui-id-web/src/components/**`, `crates/sui-id-web/src/tokens.rs`,
`crates/sui-id-i18n/src/locale/*.rs`, `crates/sui-id/src/http/router.rs`, and
the client handlers' form parsing.
**Independent design review.** [Design review record, 2026-11-15](../handoffs/149-the-console-follows-the-operator/design-review-2026-11-15.md), by the architect at `@nabbisen`'s request (*"Review them carefully by yourself with my fundamental philosophy."*), `ROADMAP.md` §S1c's owner-invoked exception.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/149-the-console-follows-the-operator/README.md`](../handoffs/149-the-console-follows-the-operator/README.md)

## Summary

Raised by the owner on 2026-11-15, after using the console:

> The path from the starting point is unclear - the menu structure is not
> carefully designed.

He also found these: *"When editing the app … part of the form has
inappropriate text input"*; table rows are tall and the audit log shows
little; the palette is unsophisticated. And:

> There is also a lack of consideration for beginners - help buttons, guide
> hovers, wizards explaining next actions, etc. - However, if you overdo
> it, the screen becomes noisy, so it is necessary to narrow down what is
> always displayed.

Measured on `7c35c42`:
- **The navigation is one flat row of seven items, in build order**
  (`layout.rs:147-157`).
- **Settings is six tabs, three of them with nothing editable.**
- **No page links to the documentation, and no page has a tooltip**
  (`title=`: zero). There are 26 field hints in all.
- The dashboard has a three-item *getting started* card (SMTP, first app,
  admin MFA: `pages/dashboard.rs:53-55`), and nothing after the setup
  wizard says what to do next.
- **An app's scopes are free text, saved without validation**
  (`handlers/admin/clients.rs:177-182`). Discovery advertises exactly four
  (`oidc/discovery.rs:68`), so a typo such as `profle` is saved, and
  silently never takes effect.
- Redirect URIs are a textarea, one per line.

**Every page works without JavaScript** (`script-src 'self'`, no inline
script: `http/security_headers.rs:66-68`). Static scripts may enhance a
page, never be required by one. This RFC keeps that.

## Decision

### 1 — navigation by task, not by build order

Six groups, in the order an operator meets them. The account menu (RFC 144)
sits apart, at the far end, because it is about the operator's identity,
not a place to go.

| Group | Contains | Today |
|---|---|---|
| **Overview** | Dashboard, with the first-run path (§2) | Dashboard |
| **People** | Users | Users |
| **Applications** | Apps | Apps |
| **Sign-in** | Federation providers; how users sign in (today's *Authentication* settings tab) | separate: nav item, and a settings tab |
| **Security** | Signing keys; sessions and lockout (today's *Security* tab); Audit log | three places |
| **System** | Language, Email, Logs, Other | Settings tabs |

**The principle: a setting lives beside what it governs.** Sign-in
settings belong with sign-in, and security settings with security. *System*
keeps only what is truly system-wide. Two layouts are drawn in the
mockups: a **grouped sidebar**, and a **grouped top bar** with menus. The
choice is §8's question 1.

### 2 — a first-run path, after the wizard

The dashboard's card becomes **an ordered path**, each step with its status
and a direct link:
1. protect your administrator account (MFA);
2. set up email, which password reset and recovery need;
3. add your first app;
4. review sign-in and session settings;
5. optionally, add a federation provider.

A completed step is shown as completed. When all are done, the path
collapses to one line, which can be reopened. **There is no second
wizard.** The setup wizard creates the first administrator; after that, a
path the operator can walk in any order is clearer than a sequence that
holds them.

### 3 — help in three layers, so the screen stays quiet

| Layer | Shown | Holds | The test |
|---|---|---|---|
| **1. Always** | on the page | the page's purpose (one line); labels; a one-line hint **only** where a field's meaning is not obvious; status words; empty states with the next step (RFC 147) | *Would a reader make a mistake without it?* Then it is layer 1 |
| **2. On request** | a **?** control beside a section, opening in place (`<details>`: keyboard, touch, no JavaScript) | up to three sentences, and **Learn more**, a link to the documentation by URL | background a reader may want once |
| **3. Reference** | the documentation | everything else | — |

**No hover-only tooltips.** A hover does not exist on touch, is hard to hold
with a tremor, and cannot be reached by keyboard. *"Guide hovers"*, which
the owner named, are met by layer 2, which opens on click or tap and stays
open. This is how the owner's caution is applied: only what prevents a
mistake is always shown.

### 4 — forms that offer choices where the choices are known

- **Scopes.** The four the server supports (`openid`, `profile`, `email`,
  `offline_access`) become checkboxes. `openid` is fixed on, because OIDC
  requires it. **Additional scopes**, for an operator's own API, become a
  separate field under layer 2. A value there that is within one edit of a
  standard scope (`profle`) is refused, with the standard name offered. A
  custom scope still works as it does today. Whether any are in use is
  §8's question 3.
- **Redirect URIs and post-logout URIs** become **one input per URI**, each
  with a **Remove** button and an **Add another**. Without JavaScript,
  *Add another* submits and the page returns with one more empty row. A
  static script may add the row in place. Each URI is checked on its own,
  so an error names the URI that failed, not *"line 3"*.
- **Create and edit are the same form**, with fields that exist only after
  creation listed and reasoned (RFC 147 §5).

### 5 — tables that show more, and read better

On RFC 144's denser cells (about 44px rows, 14px captions):
- the **audit log** keeps five columns: time, actor, action (which wraps),
  target (truncated, with its copy button), and result. Its header stays
  in view while scrolling;
- **every list states its count and its page**. None silently truncates.

### 6 — a palette that is considered, within RFC 144's rules

Two palette directions are drawn in the mockups, in both modes, on the
same components: navigation, buttons, badges, banners, a table, and a
form. Both satisfy RFC 144's five rules, and the gate will hold whichever
is chosen to them. The choice is §8's question 2. The type stays the
system sans-serif stack: an identity system is not a place for a web font
that might not load.

### 7 — what does not change

The URLs of every page (bookmarks keep working), every handler's behaviour,
and the setup wizard's steps.

## 8 — questions for the owner, answered from the mockups

> **Answered 2026-11-15, by the architect at `@nabbisen`'s request**
> (*"Review them carefully by yourself with my fundamental philosophy."*;
> [`../handoffs/149-the-console-follows-the-operator/design-review-2026-11-15.md`](../handoffs/149-the-console-follows-the-operator/design-review-2026-11-15.md)):
> 1. **the grouped sidebar**;
> 2. **palette B**, with its light-mode warning raised to `#9D6600`
>    (4.52:1);
> 3. **additional scopes are kept**, guarded as in §4. They reach the access
>    token's `scope` claim today (`oidc/tokens.rs:146`), which is how an
>    operator's own API can authorize.
>
> **The RFC was accepted the same day.**

1. **Grouped sidebar, or grouped top bar?** The architect recommends the
   **sidebar**. Six groups with labels fit there without hiding anything,
   and on a narrow screen it folds into one menu button.
2. **Which palette direction**, of the two drawn.
3. **Custom scopes.** Are additional scopes, beyond the four standard ones,
   used, or wanted? If neither, the additional field can be dropped
   entirely, which is simpler.

## What this RFC does not do

It adds no function. Editing users, password policy, and adding a
federation provider from the console are Track D, one RFC each, each with
its own freeze exception.

## Alternatives rejected

- **A wizard for everything after setup.** Rejected. A sequence that holds
  the operator is right once, for creating the first administrator.
  After that, the path of §2 lets them choose their order.
- **Tooltips on hover.** Rejected, for the reasons in §3.
- **Leave settings in one place.** Rejected as the default, because three of
  six tabs edit nothing, and the rest govern unrelated areas. A setting is
  found where its subject is. *System* keeps what has no other home.
