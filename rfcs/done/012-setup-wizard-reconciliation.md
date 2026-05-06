# RFC 012 — Setup wizard scope: spec ↔ implementation reconciliation

**Status.** Proposed
**Priority.** High. Spec compliance gap that requires a maintainer
decision before implementation.
**Tracks.** v0.29.3 codebase review — high-priority finding #4.
**Touches.** Either the spec (`docs/threat-model.md`, sec. 11.12 of
the development spec) and `docs/operators.md`, or the setup
handlers (`crates/sui-id/src/handlers/setup.rs`,
`crates/sui-id-core/src/setup.rs`, `crates/sui-id-web/src/pages.rs`).

This RFC is unusual: the change is *which way the spec and the
code line up*, not what code to write next. The maintainer makes
that call; this document scopes the options.

## Summary

The development spec, sec. 11.12, says the initial setup wizard
should collect:

- Admin account (username + password)
- Display language
- Log policy
- Encryption key (generate or inject)
- HIBP mode (off / warn / block)
- Basic operating settings

The implementation, today, collects the admin account and that's
roughly it. The setup handler's own header comment is explicit
that this is intentional — the operator's stance is "secrets
should not pass through the HTTP layer; resolve encryption before
the listener starts." That stance is defensible in its own right.

These two positions are inconsistent. They are not incompatible
— a wizard *can* skip secret-handling and still cover language /
log policy / HIBP — but the implementation skips most of the
non-secret pieces too. We have a posture mismatch that needs a
single clear answer before the next release. This RFC frames the
decision and proposes a recommendation.

## The two positions

### Position A — code is right, update the spec

The setup wizard is intentionally minimal. The encryption key is
resolved before HTTP starts (env var, key file, or generate-on-
first-run). The HIBP mode, default language, idle timeout, max
concurrent sessions, log filter — all of these have admin-UI
edit screens after setup, and changing them via the wizard would
duplicate that UI. Sec. 11.12 of the spec gets rewritten to
match this reality:

> 11.12 Initial setup
>
> The wizard collects only the initial admin's username and
> password. The master key, language, HIBP mode, and other
> server settings are resolved before HTTP starts (key) or
> have built-in defaults editable from the admin panel after
> setup (everything else). The wizard is intentionally
> minimal: anything that can be edited later is not asked at
> setup time.

**Pro:** Reflects the working code, removes the "spec says A,
code does B" tension. Smaller surface = less to test = less to
get wrong.
**Con:** Operator who reads the spec expecting a guided initial
setup is now reading a much shorter spec, and may end up running
the server with default `hibp_mode=warn` and `default_lang=ja`
without realising they could have picked.

### Position B — spec is right, extend the wizard

Add three steps to the wizard between admin-creation and done:

- Step 2.5 — display language picker (single select: Ja / En)
- Step 2.6 — HIBP mode (off / warn / block, default warn)
- Step 2.7 — basic operating settings (idle timeout, log filter)

The encryption key stays out of the wizard, regardless. The
existing rationale ("secrets don't pass through HTTP") is
correct and shouldn't be reversed.

**Pro:** Matches the spec literally. Operator gets a guided
on-ramp covering the choices most likely to need attention at
deploy time.
**Con:** More surface to maintain (~3 new pages, ~10 new i18n
keys, more e2e tests). Duplicates an existing admin-UI capability
that is just one click away after setup.

### Position C (hybrid, proposed)

Update the spec *partially* and extend the wizard *minimally*.
Specifically:

- The wizard adds **language** and **HIBP mode** as setup steps.
  Both are choices an operator is more likely to want guidance on
  at first run than to discover later.
- The wizard does **not** add log policy, idle timeout, or other
  operating settings. Those live in the post-setup admin UI
  exclusively. Spec sec. 11.12 is amended to reflect the trim.
- The encryption key handling stays exactly as the code already
  has it: resolved before HTTP starts, never in the wizard.
- The spec's existing "12-character minimum password" requirement
  for the admin password stays. (The code enforces it; the test
  suite covers it.)

**Pro:** Closes the documented gap on the two settings most
operators will want guidance on, while keeping the wizard small.
The encryption-handling principle from the code is preserved
and elevated into the spec, where it belongs as an explicit
design choice.
**Con:** Modest implementation effort. Modest spec-rewrite
effort.

## Recommendation

Position C. The reasoning:

- **Language** is the most user-visible setting. Asking once at
  setup is friendlier than defaulting to Ja and making an
  English-speaking operator hunt for the language switcher.
- **HIBP mode** is a security policy whose default (`warn`) is
  reasonable but whose three modes have meaningfully different
  trade-offs. Surfacing it at setup increases the chance that
  an operator who would have wanted `block` actually picks it.
- **Idle timeout, max concurrent sessions, log filter** all have
  defaults that work for nearly everyone and rarely benefit
  from setup-time attention. Keep them in the admin UI only.
- **Encryption key handling** is the *correct* place for the
  code to refuse the wizard's help. Promoting that into the
  spec makes the design decision visible and the gap goes
  away.

The maintainer's call. If Position A or B is preferred over C,
this RFC's design section adjusts accordingly.

## Design (assumes Position C)

### New wizard steps

The current wizard is conceptually:

```
Welcome → Create admin → Done
```

Extended:

```
Welcome → Create admin → Language → HIBP mode → Done
```

Each new step is a single page with a single decision. Both
default to a sensible value, both can be skipped (Enter / "Use
default").

#### Language step

Same UI shape as the existing `/me/security` language picker:
two radio buttons (Ja / En), default Ja, submit goes to the
next step. The chosen value is written to `server_settings.default_lang`.

i18n string keys (added to `Strings`):

- `setup_lang_title` — "表示言語の設定" / "Display language"
- `setup_lang_lede` — short description of when this is used
- `setup_lang_field_label` — "表示言語" / "Display language"
- `setup_lang_default_note` — "あとから管理画面で変更できます。" /
  "You can change this later in the admin panel."

#### HIBP step

Three radio buttons (off / warn / block), default warn,
explanatory paragraph alongside each option. The chosen value is
written to `server_settings.hibp_mode`.

i18n keys:

- `setup_hibp_title`
- `setup_hibp_lede` — explains k-anonymity, fail-open posture
- `setup_hibp_option_off` / `setup_hibp_option_warn` / `setup_hibp_option_block`
- `setup_hibp_option_off_desc` / `setup_hibp_option_warn_desc` / `setup_hibp_option_block_desc`
- `setup_hibp_default_note`

### Wizard state

The existing wizard tracks progress in a session-style
short-lived cookie. Two new fields are added: `selected_lang` and
`selected_hibp_mode`. Both have defaults applied if the user
clicks "Done" without making explicit selections (matches the
existing "skip" semantics).

### Spec amendments

In `docs/operators.md` (or wherever sec. 11.12 lives in the
delivered spec) update to:

> The wizard collects, in order: an admin username and password,
> a display language (default Ja), and an HIBP mode (default
> warn). The master key is resolved before HTTP starts and
> never passes through the wizard. Other server settings — idle
> session timeout, concurrent-session cap, log filter, SMTP
> config — have built-in defaults and are editable from the
> admin panel after setup.

The "before HTTP starts" wording matters: it documents the
encryption-key handling principle as a deliberate choice rather
than as an oversight.

## Multiple implementation steps

If the change is taken in two passes:

1. **Spec amendment + Position-A docs.** Edit sec. 11.12 to
   describe the current behaviour. Land in a docs-only release.
   This buys time and makes the discrepancy go away in the
   short term, with the option to extend to Position C later.
2. **Wizard extension.** Add the language and HIBP steps. New
   pages, new i18n strings, new e2e tests for the path through
   the extended wizard. Land in a feature release.

The spec amendment can ship without the wizard work. The
wizard work cannot ship without the spec amendment (the
delivered behaviour would still mismatch the spec, just in a
slightly less acute way).

## Tests

For Position C, the e2e test in `tests/e2e/setup_wizard.rs`
gains:

- `setup_wizard_completes_with_language_picker` — drive a
  Japanese-default and an English-explicit path, assert the
  resulting `server_settings.default_lang` matches.
- `setup_wizard_completes_with_hibp_mode` — drive each of
  off/warn/block, assert the resulting
  `server_settings.hibp_mode` matches.
- `setup_wizard_skip_uses_defaults` — click through with no
  explicit selections, assert defaults are applied.

The existing `setup_wizard_completes_minimally` test is kept
but updated to reflect the new step count (or kept in its
current shape and gated under a "minimal-setup" e2e tag if the
shorter path is also valid).

## Security considerations

None new. The two added wizard steps adjust non-secret
configuration that already has admin-UI edit screens; the
attack surface is unchanged. The encryption-key principle
remains enforced — the wizard does not see it, does not
display it, does not solicit it.

The only consideration worth naming: an attacker who has
intercepted the setup token can already complete the wizard
and become the initial admin. That hasn't changed. The new
steps don't increase the attacker's leverage; they just
extend the form they would have submitted anyway.

## Open questions

- **Should the wizard's HIBP step include a "test the connection"
  button?** Probably not — adds complexity and HIBP is
  fail-open at runtime, so a test-connection mismatch isn't
  meaningful. Leave it out.
- **Is the spec's "log policy" item important enough to add to
  the wizard?** Recommend no. The default log filter is
  appropriate for nearly all deployments and a misconfigured
  log filter at setup is a noisy problem with an obvious fix
  ("change it in the admin panel").
- **Does the language picker need to also cover the chosen
  language for the *Done* page itself?** Yes — the Done page
  should already be rendering in the just-selected locale.
  Mechanical wiring; falls out of the existing
  `RequestLocale` extractor.
