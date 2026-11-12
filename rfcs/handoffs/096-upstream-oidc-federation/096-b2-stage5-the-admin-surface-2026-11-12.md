# RFC 096-B2 stage 5 — the federation-provider admin surface

> ## Landed 2026-11-12, with reviewer fold-ins. Next: two decisions —
> [`096-b2-login-screen-buttons-decision-2026-11-12.md`](096-b2-login-screen-buttons-decision-2026-11-12.md).
>
> Cycle L, 096-B2 S5 —
> [`096-b2-stage-plan-2026-11-11.md`](096-b2-stage-plan-2026-11-11.md) §3.
> **This document is the dispatch.**

**Dispatched.** 2026-11-12, by the architect.
**Governing RFC.** [RFC 096](../../accepted/096-upstream-oidc-federation-validation.md),
**Accepted**, §096-B2: the admin surface (`:379-391`), and the C17-seam
amendment (`:390`, `:440`). Plus the UI-security contract, RFCs 088–092 (in
`rfcs/done/`). Building is permitted.
**Baseline.** The tip of `main` when you start (`git log -1`; record it). S4
must be in it, with its review's fold-ins.
**Read first:**
- the approved design, [`096-b2-c17-consumes-the-capability-decision-2026-11-11.md`](096-b2-c17-consumes-the-capability-decision-2026-11-11.md)
  §3, items 2 and 6: one "Check and enable" action, and every refusal with a
  next step;
- the S4 review's §3, `.git-exclude/reviewed/096-b2-stage4-c17-enable-and-disable-2026-11-12.md`,
  for the entry points;
- **the precedent to copy, not reinvent**: the signing-keys pages
  (`crates/sui-id/src/http/handlers/admin/signing_keys.rs`, and their routes
  in `router.rs`): a list page, a step-up-gated confirm page, then the POST.

## 0 — what this stage is

**After this stage an administrator can see every federation provider's
state and act on it, in one place.** That is cycle L's third protection
(`cycle-l-and-0.90.0-2026-11-11.md` §4): *"the admin UI shows the state and
the action in one place"*. **No trust field is editable** (RFC 096 `:380`,
*"admin UI may enable or disable but may not mutate trust fields"*). This
stage builds no delete and no unlink; those are S6.

## A — pages and routes

| Route | Who | What |
|---|---|---|
| `GET /admin/federation-providers` | admin and auditor (RFC 088: read-only for an auditor) | one row per provider: display name, slug, issuer, **state** (enabled or disabled), **review status**, when it was last checked and enabled (`evidence_observed_at`, or "never"), and whether it has linked users. The action buttons are shown to admins only. A provider with no configuration block (A2's C25 case) says so in its row, and **offers no Enable button**, because that action could only fail. The row says what to do instead: restore the block and restart |
| `GET …/{id}/enable-confirm` | admin; auditor 403; **step-up** | states what will happen, in operator words: *sui-id will contact the upstream now, check its discovery document and signing keys, and enable the provider only if every check passes*. Then two buttons: **Check and enable**, and **Check only** |
| `POST …/{id}/enable` | admin; CSRF | `check_and_enable`. Success goes to the list with a success flash. A refusal re-renders the confirm page with that refusal's message (§B). **`StepUpRequired` is the step-up redirect, not a message** |
| `POST …/{id}/check` | admin; CSRF; **no step-up**, since it writes nothing | `preflight_provider`, and the capability is dropped. Re-renders the confirm page with either "every check passed" or the refusal |
| `GET …/{id}/disable-confirm` | admin; auditor 403; **step-up** | the impact, in operator words: *federated sign-in through this provider stops now; sign-ins already in progress end; users keep their links* |
| `POST …/{id}/disable` | admin; CSRF | `disable_federation_provider`. Already disabled is a neutral flash, not an error. **`StepUpRequired` is the redirect** |

- **Step-up return.** Add `/admin/federation-providers/` to
  `STEP_UP_RETURN_ALLOWLIST` (`handlers/step_up.rs:61`). RFC 089 P1 makes
  that list the only place a post-step-up return may go, and P5 forbids
  auto-execute: after step-up the administrator lands on the confirm page,
  and only its POST acts. A test proves each.
- **Navigation.** One entry in the admin chrome, next to signing keys.

## B — every message, in both languages

The UI is Japanese and English (`Locale::ALL`, `crates/sui-id-i18n/src/lib.rs:97`).
The `Display` texts of `PreflightError`, `CheckAndEnableError` and
`DisableFederationProviderError` are English, and **stay as they are, for
logs**. The UI shows a translated message:
- **one i18n key per variant**, in Japanese and English, each keeping the
  reason **and** the next step;
- **one exhaustive `match`**, from variant to key, so a new variant fails to
  compile until it is translated;
- the `text-leaks` check (`contracts/ui-invariants.toml`) must stay green,
  meaning no hard-coded user-facing English in a page.

A test renders the confirm page for every refusal in both locales, and
checks that the reason and the next step appear in each.

## C — the operator messages S1 to S2b left for this stage

Three `TODO(096-B2 S5)` markers in `crates/sui-id/src/runtime/startup.rs`
(`:381`, `:538`, `:620`) name a mechanism that did not exist yet. **Name the
real one now**: the page path, `/admin/federation-providers`, and what to
press. Update each message's test to assert the new text. `grep -rn
"TODO(096-B2 S5)" crates` must find nothing afterward.

## D — tests

- **The list**, as admin and as auditor. The auditor sees no action button,
  and gets 403 on every confirm page and every POST.
- **CSRF**: each POST without a token is refused.
- **Step-up**: each confirm page without a fresh step-up redirects to it,
  and after step-up returns to that confirm page. Nothing executes until
  the POST (RFC 089 P5).
- **End to end, through the HTTP surface**, against the mock upstream:
  - enable from the confirm page;
  - **the audit row** `federation.provider.enabled`, with the
    administrator as actor;
  - **a real federated sign-in through the provider**;
  - disable from its confirm page, with the audit row
    `federation.provider.disabled`;
  - the login screen no longer offers it.
- **Every refusal renders**: one test per variant, through the page, in
  both locales (§B).
- **No trust field is editable.** No form on any of these pages has a field
  for any of the digest's nine fields. Assert it by the absence of `<input>`
  and `<select>` names other than the CSRF token.

## E — records

- **`CHANGELOG.md` `[Unreleased]`.** This is the first stage an operator can
  reach, so it gets the entry, in operator words, under **`### Added`**:
  - the page, and what it shows;
  - check and enable;
  - check only;
  - disable.

  And under `### Changed`: the startup messages now name the page.
- **Operator guide:** the full guide is S7. Here, make the smallest change
  that is true, wherever `docs/src` says enabling has *"no mechanism"* or
  *"a mechanism this release does not yet provide"*. `grep -rn` both phrases.
- `ui-invariants.toml`'s blocking checks stay green. G23 stays green.

## F — constraints and what to return

- **No tag, no dispatch.**
- Run `bash scripts/local-matrix.sh --changed`, the complete matrix, and
  quote the scope line and any failing check names.
- **Don't edit files while the matrix reads them.**
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/096-b2-stage5-the-admin-surface-2026-11-12.md`,
with:
- the baseline;
- §A's table, with each route's handler `file:line` and its tests;
- the allowlist change and its tests;
- the variant-to-key `match`, and both locales' text for every refusal;
- the three startup messages' new text;
- the end-to-end test, with its audit-row assertions;
- **a screenshot-free description of each page**: every element, in order,
  as an administrator meets it.
