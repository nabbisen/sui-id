# RFC 144 — The interface is legible to older eyes

**Status.** Accepted
**Accepted on.** 2026-11-15
**Approved by.** `@nabbisen`, 2026-11-15: *"All accepted."* His acceptance of 2026-11-04 (*"Accepted."*) covered the original design; this one covers the 2026-11-15 refresh.
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** *(Raised 2026-11-15 to the stricter standard; see the refresh section.)* Every foreground/background **pair a component actually composes** clears WCAG 2.1 **AAA (7:1; 4.5:1 for large text)** in both modes, and every control boundary and the focus ring clear 3:1 — a token has no ratio, a pair does, and `--danger-default` clears AA on white while failing on `--danger-subtle`; the contrast assertion in `crates/sui-id-web/src/tokens/tests.rs` covers component stylesheets and not only `TOKENS_CSS`, with no token excluded from it on the grounds that it is decorative; the border token used on interactive controls clears 3:1 (WCAG 1.4.11); the type scale is expressed in `rem` so a reader's own browser font size is honoured, with a test that fails if a `px` font size returns; interactive controls clear 44×44 CSS px; and `prefers-contrast: more` is honoured and asserted by the same gate that asserts `prefers-reduced-motion`. **And (2026-11-15):** no `placeholder` attribute carries guidance, asserted by a test that fails on any `placeholder=` in a page; and every page that shows the navigation renders the account menu, asserted by a test over every `show_nav=true` page.
**Tracks.** UI/UX accessibility. §5 and §6 carry open questions.
**Touches.** `crates/sui-id-web/src/tokens.rs`, `crates/sui-id-web/src/tokens/tests.rs`, `crates/sui-id-web/src/components/forms.rs`, `crates/sui-id-web/src/components/buttons.rs`, `crates/sui-id-web/src/components/chrome.rs`, `crates/sui-id-web/src/components/setup.rs`.
**Independent design review.** [Design review record, 2026-11-04](../handoffs/144-legible-to-older-eyes/design-review-2026-11-04.md), by the architect at `@nabbisen`'s request (*"Review it carefully by yourself with my fundamental philosophy"*), `ROADMAP.md` §S1c's owner-invoked exception. **And for the 2026-11-15 refresh:** [design review record, 2026-11-15](../handoffs/144-legible-to-older-eyes/design-review-2026-11-15.md), `@nabbisen`'s acceptance, in his words.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/144-legible-to-older-eyes/README.md`](../handoffs/144-legible-to-older-eyes/README.md)

## Summary

Raised by `@nabbisen`, 2026-11-04: *"Are there some points to improve or
enhance especially for elder people?"* — of the view layout, typography
and colour. Measuring the live
token source against the components that consume it found **two failures
of the standard this project already claims to meet**, and four
enhancements that are not failures but matter disproportionately to this
population.

The sharpest finding is not a colour value. It is that
`crates/sui-id-web/src/tokens/tests.rs` is an executable contrast
contract — *"a token edit that drops any text-on-colour pair below AA
fails the build"* — which **excludes the one token that fails**, on the
written grounds that it is *"not a text carrier"*. Three live call sites
set that token as `color:`, one of them on `input::placeholder`. The
gate reports green because its prose was checked and its readers were
not.

## Why this is an RFC

The design tokens are consumed by every component and guarded by a gate
with a stated contract. Changing them changes that contract, so it is a
design decision rather than a repair. Two items below (§5, §6) are
judgement calls written as questions, not as decisions already taken.

## Decision

### 1 — the contrast contract covers the palette, not what components compose

| mode | pair | measured | AA requires |
|---|---|---|---|
| light | `#9A94A3` on `#F6F5F9` | **2.71:1** | 4.5:1 |
| dark | `#7E788A` on `#1A1720` | **4.16:1** | 4.5:1 |

Live `color:` call sites, each verified:

- `components/forms.rs:43` — `input::placeholder`, `textarea::placeholder`
- `components/chrome.rs:285` — `.app-footer__version`
- `components/setup.rs:142` — `.setup-step__label--upcoming`

(`components/chrome.rs:86`'s `.subtle` utility class has **zero**
usages; it is dead and should be deleted rather than fixed.)

**Placeholder text is the one that matters.** It is frequently the only
hint about what a field expects, and it is being painted below AA.

**And the exclusion is the smaller half.** `tokens/tests.rs` reads
`TOKENS_CSS` and nothing else (`:19`, `:97`), so it never sees
`CONFIRM_CSS`, `BANNERS_CSS` or any component stylesheet. The contract
covers pairs the **token file** declares; a component may put any
token's colour on any other token's background and nothing checks it.
Three such pairs fail AA today — §1a.

**Decision.** Placeholders get their own AA-compliant token rather than
sharing a decorative one — a placeholder is content. The remaining uses
move to a value clearing 4.5:1 in both modes. The exclusion is removed.
**And the assertion is widened to the pairs components compose**, which
is the mechanism rather than the symptom.

### 1a — three failures in destructive-action UI

All in `components/confirm.rs`, all measured:

| element | pair | measured | size |
|---|---|---|---|
| `.reversibility-badge--permanent` (`:31-32`) | `--danger-default` on `--danger-subtle` | **4.28:1** | 13px |
| `.reversibility-badge--recoverable` (`:27-28`) | `--success-default` on 15% success over surface | **3.80:1** | 13px |
| `.danger-zone__title` (`:48-51`) | `--danger-default` on `--danger-subtle` | **4.28:1** | 15px |

The permanence badge is the label saying *this cannot be undone* (RFC
017 §3); the danger-zone title heads the section whose own comment says
it *"physically and semantically isolates destructive operations"*. The
two places built to stop an accidental destruction are below the
standard, one at caption size.

**`components/banners.rs` already does it correctly** — every tinted
background pairs with `color: var(--fg-default)`, **13.72:1 to
15.06:1**, with the semantic colour on the border. `.danger-zone__body`
does the same. **Decision: apply that existing convention** to the three
failures rather than invent a palette; tint the background, keep text at
full foreground contrast, carry meaning on the border.

*This also answers where the problem was not. The "messages areas" are
sound; the labels are not.*

### 2 — control borders are effectively invisible

`--border-default` is `#D6D1DE` (light) and `#3A3445` (dark) against
page surfaces `#F6F5F9` / `#1A1720`: **1.38:1** and **~1.5:1**. WCAG
1.4.11 asks 3:1 where a control's visual boundary is what identifies it.

**Decision.** Split the token: a control border clearing 3:1, and a
divider token that may stay subtle for rules and table lines. One change
rather than one per component.

### 3 — the type scale honours the reader's own setting

Every size in `tokens.rs` is `px` — sixteen values, body at `15px`,
caption at `13px`. Browser **zoom** works, so WCAG 1.4.4 is satisfied;
but a reader who has set a larger default font size in their browser
sees **no change at all**. That setting is among the most common
accommodations older readers make.

**Decision.** Express the scale in `rem`, body at `1rem` (16px, the
platform default, where it is currently 15px). Add an assertion that
fails if a `px` font size reappears — the gate that exists for contrast
should also hold this, or the regression is silent.

**The spacing scale must move with it, or the RFC must say why not.**
`--space-1` through `--space-6` are `4px`–`48px`. Convert type alone and
a reader's enlarged font grows the text while the padding holds still,
crowding every line against its container — an accommodation that
partly defeats itself.

### 4 — interactive controls reach 44×44

`components/buttons.rs:28` sets `min-height: 36px`. WCAG 2.5.8 (AA,
24×24) passes; **WCAG 2.5.5 (AAA, 44×44) does not**, and 44 is also both
major platform guidelines. Reduced precision and tremor are the common
case here, and a mis-tap on a destructive control costs more than a
mis-tap elsewhere.

**Decision.** 44×44 for **standalone** controls — buttons, form
controls, menu items. **Not inline links in prose**, which WCAG 2.5.8
exempts and which a blanket rule would wrongly inflate.

### 5 — `prefers-contrast`, and how far to go on AAA

`prefers-reduced-motion` is honoured (`tokens.rs:288`) and gate-asserted.
**`prefers-contrast` is not supported at all.** A reader who asks their
operating system for more contrast gets nothing from this interface.

**Decision.** Honour `prefers-contrast: more`, asserted by the same
gate.

**Open, and `@nabbisen`'s:** every semantic colour passes AA and none
reaches AAA — primary `5.69:1`, danger `5.29:1`, warning `5.33:1`,
success `4.98:1`, info `5.27:1`, muted body text `6.16:1`. AA is the
standard. AAA is the standard that assumes the reader is not at their
best, which is a fair assumption for an identity system someone may be
using while locked out. Raising the semantic set to 7:1 shifts the
palette's character, so the architect proposes it and does not assume
it.

### 6 — what is already right, and is not changing

Stated so this is not read as a rewrite. Body text is **15.60:1** light
and **15.41:1** dark. Line heights are generous and correct — body 1.6,
caption 1.5. Disabled states use explicit AA tokens with the ratio
recorded beside the value, rather than opacity. The account menu is
`<details>`/`<summary>` and needs no JavaScript. None of this is touched.

**Open, and `@nabbisen`'s:** placeholder text disappears the moment
typing begins, which is when an uncertain reader most wants it, and
`pages/confirm.rs:143` puts one on a textarea on a confirmation screen.
Darkening it is strictly an improvement; not relying on it for guidance
that matters is a product decision.

**Open, and `@nabbisen`'s:** the account menu puts sign-out two actions
away behind a `username ▾` summary. For an administrator that is a
reasonable trade; for someone wanting to be certain they have signed out
of a shared machine it is less kind. Recorded as a question because it
is a product judgement, not an accessibility failure.

### 7 — what this RFC does not claim to answer

The layout and flow half of the question — whether menu positions suit
an older reader, whether the setup flow's order matches expectation,
whether error recovery reads as reassuring — **cannot be answered by
reading source**. Contrast and size are measurable; comprehension is
observed. That work is a different kind and is not scoped here.

## Refresh, 2026-11-15: toward the stricter standard

**Asked.** `@nabbisen`, on the operator-experience assessment's decision 3:
*"Seems the stricter are preferred, for easiness to view and read is very
important. However, review them carefully with my fundamental
philosophy."* His philosophy has two halves: clean, safe, robust and
sophisticated; **and** an interface its users cannot misunderstand. Both
are applied below. Every figure was computed on `7c35c42`'s tokens: WCAG
2.x relative luminance, against **both** page surfaces in each mode
(`--surface-default` and `--surface-elevated`), the lower ratio reported.
The method, the script and its full output are in [`../handoffs/144-legible-to-older-eyes/aaa-measurement-2026-11-15.md`](../handoffs/144-legible-to-older-eyes/aaa-measurement-2026-11-15.md).

### R — five rules, in place of AA's one

1. **All text reaches 7:1** (WCAG 1.4.6, AAA), in both modes; text 24px
   and up, or 18.66px bold, reaches 4.5:1. That includes button labels,
   badges, captions and the footer, and nothing is exempt on a claim about
   its use, the defect §1 found.
2. **Meaning is never carried by colour alone** (WCAG 1.4.1). Every status
   has a word, and a tinted surface with a coloured border or icon: the
   convention `components/banners.rs` already follows (§1a).
3. **A semantic colour is used as text only where the text is itself the
   signal**: links, the current navigation item, and field errors. So only
   **accent** and **danger** are ever text, and those two are also the only
   button fills (`--fg-on-accent`, `--fg-on-danger`: one user each).
   Warning, success and info are **never text and never fills**, only
   borders, icons and tints.
4. **Every control boundary and the focus ring reach 3:1** (WCAG 1.4.11).
   WCAG has no stricter level for non-text, and none is invented here.
5. **`prefers-contrast: more`** (§5) raises control borders to 4.5:1, and
   draws a solid border wherever a tint alone marks a state.

### The values

Hue is kept; only OKLab lightness moves (chroma falls only where sRGB's
gamut forces it).

| Token | Light now | Light, proposed | Dark now | Dark, proposed |
|---|---|---|---|---|
| `--accent-default` | `#6956C0`: text 5.24, label on it 5.69 | **`#5741AA`**: 7.06 / 7.66 | `#A89BFF`: 6.29 / 7.41 | **`#B2A6FF`**: 7.00 / 8.25 |
| `--danger-default` | `#BE3F3F`: 4.87 / 5.29 | **`#A12127`**: 7.00 / 7.60 | `#FF6B6B`: 5.41 / 6.37 | **`#FF938F`**: 7.02 / 8.27 |
| `--fg-muted` | `#5F5A66`: 6.16 | **`#56515D`**: 7.09 | `#B8B2C4`: 7.30 | unchanged |
| control border (§2) | `#D6D1DE`: 1.38 | **`#918C98`**: 3.02 | `#3A3445`: 1.26 | **`#736D80`**: 3.02 |
| `--warning-default`, `--success-default`, `--info-default` | 4.59–4.91 as non-text | **unchanged**: rule 3 makes them non-text, where 3:1 applies | 6.24–8.13 | **unchanged** |

Body text is already **15.60:1** light and **13.09:1** dark, and does not
move. *(These figures are lower than some in §1–§6, which measured against
`--surface-default` alone. The refresh takes the lower of the two surfaces,
because text sits on cards as well as on the page. For example, dark
`--fg-muted` is 8.59:1 on the page and 7.30:1 on a card.)*

### Measured against the first half: clean, robust, sophisticated

- **One rule set, no exemptions.** The gate asserts rule 1 over every pair
  a component composes (§1's widening), and rule 4 over every boundary. A
  token cannot be excused as *"not a text carrier"* again, because the gate
  no longer takes a token's word for it.
- **Fewer moving parts than AA.** Rule 3 means only two semantic colours
  carry text, so only two had to move for text. Warning stays **amber**. At
  7:1 as text it would have turned brown (`#714D00`) and stopped looking
  like a warning. Not making it text is what keeps it readable as a
  warning.
- **The cost, stated:** in light mode, accent and danger become deeper and
  less vivid, and the semantic colours sit nearer to body text (OKLab ΔE
  0.315 → 0.229). Rule 2 is what makes that safe: a status is read from
  its word and its surface, not from the hue of its letters.
- **In dark mode**, danger becomes a lighter red (`#FF938F`), which reads as
  less alarming by hue alone. Rule 2 again carries the meaning.
- **This sets a floor, not a look.** Your assessment's A9, that the palette
  is unsophisticated, is a design question this RFC does not settle. A
  palette refresh belongs to the operator-experience track's
  visual-structure RFC, designed with mockups you review. It must satisfy
  these five rules, which the gate will then hold it to.

### Measured against the second half: nothing to misunderstand

- **The colours are easier to tell apart, not harder.** The closest pair
  of semantic colours, accent and info, moves from ΔE **0.094 → 0.119**
  (light) and **0.074 → 0.082** (dark). Darkening accent alone separated
  it from info.
- **Placeholders are removed, not darkened** (§6's first question). A
  placeholder at 7:1 looks like text someone has already typed, which is a
  way to misunderstand a form. And it vanishes the moment typing begins,
  when an uncertain reader most wants it. Six exist. **Two carry guidance
  that matters** and that disappears as the user types: the SMTP password's
  *"(only enter to change)"* and *"(leave blank if no authentication)"*
  (`pages/settings/email.rs:45-48`). Four are examples (*"e.g. auth.login"*,
  *"e.g. YubiKey 5C"*, …). **Decision proposed:** every one becomes visible
  hint text under its field (`field__hint`, which reaches 7:1 under rule 1
  and stays while typing), and the `placeholder` attribute is not used.
  §1's placeholder token is then no longer needed.
- **Sign-out lives in the account menu, on every page** (§6's second
  question). Measured 2026-11-15: **38 of 39 pages** never received RFC
  074's menu, and render a bare *Sign out* directly after *Settings*
  (`layout.rs:171-207`). That is the misplacement the owner found, and
  *"increasing the risk of users pressing the wrong buttons"* is his own
  word for its cost. **This RFC's §6 claimed the menu as already right; it
  was right on the dashboard only. That claim was the architect's, and
  unmeasured.**
  **Decision proposed:** the account menu (`username ▾` → *My account*,
  *Sign out*) on every page, set apart from the main navigation, because it
  is about identity, not a place to go. After signing out, a page that says
  plainly *you are signed out*, which answers the shared-machine worry §6
  raised: certainty comes from that confirmation, not from a button within
  reach.

### What else moves, so the rules hold

- `.badge--ok`, `--warn` and `--danger` are coloured text on the page
  surface (`components/badges.rs:37-39`). They take the banner convention:
  a tinted fill, text in `--fg-default`, and a coloured border.
- `components/banners.rs:149` and `components/confirm.rs:28` set success as
  text. Under rule 3 they move to `--fg-default`.
- `--fg-subtle` (2.71 light) is never text again. The footer version and the
  setup wizard's upcoming step (§1) use `--fg-muted`.
- **Type and density together** (§3, and the assessment's A8): body
  `1rem`; caption `0.875rem` (14px, up from 13px); table cells padded
  `--space-2` (8px), not `--space-3` (16px), vertically. An audit row then
  stands at about **44px**, which is also §4's target size, instead of
  about 57px. The text is larger, and more rows fit on the screen.

### What stays open

No question remains open in this RFC once the refresh is accepted. §5's and
§6's questions are answered above, as proposals.

## Alternatives rejected

**Raise `--fg-subtle` and keep the gate's exclusion.** Rejected: the
exclusion is the defect. A contrast contract that exempts a token on an
unverified claim about its use will drift again the moment someone uses
the exempt token for text, which is exactly what happened.

**Leave the type scale in `px` and rely on zoom.** Rejected: zoom and
font-size preference are different accommodations. Honouring only the
one that happens to work is the kind of partial compliance this project
records as a divergence rather than ships quietly.

**A `@media (prefers-contrast)` block alone, without fixing §1 and §2.**
Rejected: it would make the interface legible only for readers who know
the setting exists and have found it.
