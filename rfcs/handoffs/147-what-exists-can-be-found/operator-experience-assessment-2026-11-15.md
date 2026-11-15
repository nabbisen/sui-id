# The operator's experience — first run, the command line, and the admin console

**Measured 2026-11-15, by the architect**, on the `0.91.0` binary
(`a0a6c1a`) and its source. This answers `@nabbisen`'s two messages of
2026-11-15: the admin console, then the first run and the command line.
Every row below was measured: run, read at `file:line`, or both. Opinion is
marked as opinion.
**Whose turn.** §6 is decided. Nothing here is dispatched.
**Status.** An assessment, not a plan. **§6 was decided 2026-11-15**
(ROADMAP, *Active plan*): the track runs before M6, and repairs count as
repairs under the freeze. RFC 144 is reviewed toward the stricter standard,
and nothing is built before the schedule and every RFC of the track exist.
That work became RFCs 144 (refreshed), 147, 148 and 149, and cycles N–P.
Moved into git the same day.

## 1 — what you raised, and what is there

**Kind:** **D** is a defect: it is wrong as built, or contradicts its own
design. **F** is findability: the function exists, but cannot be found.
**M** is missing: it does not exist. **L** is legibility. **S** is structure:
how things are organised.

### The admin console

| # | You raised | Measured | Kind |
|---|---|---|---|
| A1 | Users cannot be added | Adding exists. Its **only entry point is an icon-only "+"** with no visible text; the label is in `aria-label` only (`sui-id-web/src/pages/users.rs:90-93`) | **F** |
| A2 | Users cannot be edited | True. A user can have their role changed, be disabled, have MFA reset, get a recovery link, or be deleted. **Nothing edits a username, display name or email** (`sui-id/src/http/router.rs:159-195`) | **M** |
| A3 | Nothing to do on the federation provider screen at first | True as an operator sees it. With no config block, the page says *"No federation providers are configured yet."*, with no action and no hint that a provider is added in the config file and a restart (`federation_providers.rs:144`). A provider cannot be added from the UI at all | **F** + **M** |
| A4 | Settings: little to do; the minimum password length cannot be changed | **Three of six tabs (Authentication, Logs, Other) have no editable field.** Basic has the default language only; Security has idle timeout and max sessions. Minimum password length is fixed at 12: production always runs `SecurityLevel::Standard` (`runtime/state.rs:121-135`), and neither the UI nor the config file sets it. The enum was a refactor of two constants (CHANGELOG `:3775-3807`), not a ruling that policy is fixed | **M** |
| A5 | The path from the start is unclear; the menu is not designed | The navigation is **one flat row of seven items** in build order: Dashboard, Users, Apps, Signing keys, Federation providers, Audit, Settings (`layout.rs:147-157`). There is no grouping by task. The dashboard does have a three-item *getting started* card (SMTP, first app, admin MFA) and warning cards (`pages/dashboard.rs:40-75`; `has_action_items` at `:68`) | **S** |
| A6 | "Settings" and "Sign out" sit too close together | **A defect, with a measured cause.** RFC 074 replaced the flat sign-out with a user menu. But **no page passes `admin_username` to the shell**, so **38 of the 39** pages that show the navigation render the old fallback: a bare *Sign out* styled as a nav link, directly after *Settings* (`layout.rs:171-207`). On narrow screens the gap is one 4px step (`chrome.rs:446`). Only the dashboard, which builds its own navigation, shows the menu | **D** |
| A7 | Some app-form fields are free text where a choice is right | **Scopes are one text field** (`allowed_scopes`, default `openid profile email`). **Redirect URIs are a textarea**, one per line, with no add or remove controls (`pages/clients.rs:211-216`). Homepage, privacy and terms URLs are `type="text"`, not `type="url"`. *Consent policy* is on the edit form, but not the create form (`:417`) | **S** / **D** |
| A8 | Row height; little of the audit log fits on screen | Each row is about **57px**: 16px padding above and below, and 15px text at line-height 1.6 (`components/tables.rs:52-61`, `tokens.rs:94-109`). About ten to twelve rows fit on a laptop screen. IDs are 13px monospace | **L** |
| A9 | The palette is unsophisticated | Opinion, and mine agrees: the tokens are those adopted from mockup `v0.4.8` in the `v0.49`–`v0.57` arc and not revisited since. Measured: RFC 144 found two WCAG AA failures in what components compose, and none of the semantic colours reaches AAA | **L** |
| A10 | Not enough for poor eyesight and the elderly | **RFC 144** answers part of it. It was written on your question of 2026-11-04, and has sat in `proposed/` since, waiting on your answers to §5 and §6 | **L** |
| A11 | Little for beginners: help, hovers, wizards; but not too noisy | **No page links to the documentation, and no page has a tooltip** (`title=`: zero). There are 26 field hints. The setup wizard exists, but nothing guides the operator after it | **M** |

### The first run and the command line

| # | You raised | Measured | Kind |
|---|---|---|---|
| C1 | A missing `sui-id.toml` gives no guidance | `sui-id` in an empty directory prints only *"failed to read config ./sui-id.toml: No such file or directory"*. **And since `0.89.0`, a user who downloads the binary has no `sui-id.example.toml`**: the release ships only the binaries and `SHA256SUMS`. So the message must name `--print-sample-config`, and mention the example file only for source checkouts | **D** |
| C2 | It is unclear that `setup` needs `sui-id.toml` | `sui-id setup --admin-username admin` fails with the same bare message. **`setup` is missing from `--help`'s USAGE list**, though it is described at help line 117 | **D** |
| C3 | Offer to create `sui-id.toml` when it is missing | Not offered. The Quick Start teaches `sui-id --print-sample-config > sui-id.toml` (`quick-start.md:108`), but the binary never does | **M** |
| C4 | The commands, subcommand hierarchy and help need organising | `--help` is **190 lines**, one flat list, not grouped by task. Modes are mixed in as flags: `--dev`, `--print-sample-config`, `--version`. `sui-id help` is an error. **`sui-id.example.toml` (119 lines) and `--print-sample-config` (28 lines) disagree**: the printed one has `access_log`, `federation_providers`, `metrics_*` and `user_sources`, which the example lacks | **S** / **D** |
| C5 | `--print-sample-config` belongs to no subcommand, and its purpose is unclear | True: it is a flag, explained only as *"Print a sample configuration and exit."* (help line 176, from `cli.rs:1162`). It never says *"this is how you create `sui-id.toml`"* | **S** |

## 2 — found while measuring, beyond what you raised

- **RFC 144's §6 claims the account menu as already right** (*"The account
  menu is `<details>`/`<summary>`"*). That is true only on the dashboard (A6).
  RFC 144 is the architect's, so this is the architect's error.
- **The live-run runbook's steps 3–4 are slightly wrong.** *Check only* is on
  the confirm page after *Check and enable*, not in the row
  (`federation_providers.rs:56-64`, `:197-205`). The architect's error.
- RFC 107 (accepted) fixes only *"`--help` lists every subcommand"*. It does
  not touch the structure C4 is about.

## 3 — why it is like this

1. **Product work has been frozen since the remediation programme began**
   (planning baseline `v0.76.12`, 2026-07-16): *"New feature expansion …
   remain[s] frozen until every milestone … has passed its exit gate"*
   (ROADMAP, *Active plan*). The last UI arc ended near `v0.60`. A2, A4, A11
   and C3 are features, and the freeze held them back.
2. **No one owns the experience as a whole.** Changes land one RFC at a
   time, and nothing checks the sum. RFC 074's menu reached one page (A6),
   and nothing noticed for over thirty releases.
3. **The gates check written contracts, not use.** RFC 144 found a contrast
   gate that excluded the one failing token. RFC 107 checks that the help
   *lists* every subcommand, not that a newcomer can find their way.
4. **No one walked the product as a newcomer, the architect included.** This
   is the first time the architect ran the first run from an empty directory.
   The stage 14 runbook assumed knowledge a first-time operator lacks.
   RFC 144 §7 had said as much: *"comprehension is observed"*. You have now
   observed it.

## 4 — the constraint that decides *when*

**M6 builds one immutable artifact, and M7 soaks it for at least four
meaningfully exercised weeks.** A change after soak entry may reset the
window, under the ROADMAP's *M7 soak workload and reset rules*.

So operator-experience work has two possible homes:
- **before M6**, which needs your freeze exception, and moves M6 and M7 later;
- **after M7**, which keeps the remediation programme's dates. But then M7
  soaks an admin console that cannot edit a user. A soak of *"representative
  auth traffic … operational cycles"* is then less representative than it
  claims.

**In between is the costly place**: each change could reset the soak.

## 5 — a shape, not yet a plan

Four tracks, separated because they need different decisions:

| Track | Contents | What it needs |
|---|---|---|
| **A — repairs** | A6 (the user menu on every page); A1 (a labelled *Add user* button); A3's empty state saying how to add a provider; C1 and C2 (messages that say what to do, with `--print-sample-config` named); `setup` in USAGE; `sui-id help` as an alias; `type="url"`; A7's create/edit mismatch | Small. They are **repairs** under the freeze's own exception, *"repairs … to keep approved gates executable"* and contract defects, **if you classify them so**. It could ride `0.92.0` beside RFC 100 |
| **B — legibility** | RFC 144, plus table density (A8) | Your answers to RFC 144's §5 (AAA or AA) and §6 (placeholders, sign-out placement), then its acceptance |
| **C — structure** | **The command line**: one command system, with subcommands for every mode (`serve`, `init` that offers to write `sui-id.toml`, `setup`, `config`, `admin …`, `help`), the old flags kept as aliases, and one sample config, so C4's disagreement cannot recur. **The admin information architecture**: navigation grouped by task, a first-run path after the setup wizard, and a policy for help (what is always shown, what only on request; your caution about noise) | Design first, as RFCs **with mockups you review before anything is built**. Not quick |
| **D — missing features** | A2 (edit a user); A4 (password policy, and the settings tabs that show without allowing change); adding a federation provider from the UI (A3, which needs a design for its secret); A7's selectors for scopes; A11's in-context help | A **freeze exception per item**, or after M7 |

## 6 — your decisions

1. **When**: before M6, as a track beside the remediation (needs your freeze
   exception, and moves M6/M7), or after M7. **My recommendation is before
   M6, but only Tracks A–C at first, with D chosen item by item.** It is your
   philosophy's second half, and the soak should exercise the product an
   operator will actually use. Doing it later costs a soak reset each time.
2. **Track A as repairs** under the freeze's exception. Recommended. Each is
   a defect against a contract the project already holds (RFC 074, RFC 107,
   the configuration reference, the Quick Start).
3. **RFC 144's two open questions**: AAA or AA for the semantic colours
   (§5), and placeholders and sign-out placement (§6).
4. **The order within Track C**: command line first, which is the smaller and
   the first thing a newcomer meets; or the admin console first.

**The live run (stage 14)** is paused, not lost: the instance is still up on
`127.0.0.1:8802`. It does not depend on any of this. Whether to finish it
now or after these decisions is also yours.
