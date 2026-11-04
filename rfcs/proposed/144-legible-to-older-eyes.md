# RFC 144 — The interface is legible to older eyes

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** Every token used as `color:` clears WCAG 2.1 AA (4.5:1) in both modes, with no token excluded from `crates/sui-id-web/src/tokens/tests.rs`'s contrast assertion on the grounds that it is decorative; the border token used on interactive controls clears 3:1 (WCAG 1.4.11); the type scale is expressed in `rem` so a reader's own browser font size is honoured, with a test that fails if a `px` font size returns; interactive controls clear 44×44 CSS px; and `prefers-contrast: more` is honoured and asserted by the same gate that asserts `prefers-reduced-motion`.
**Tracks.** UI/UX accessibility. §5 and §6 carry open questions.
**Touches.** `crates/sui-id-web/src/tokens.rs`, `crates/sui-id-web/src/tokens/tests.rs`, `crates/sui-id-web/src/components/forms.rs`, `crates/sui-id-web/src/components/buttons.rs`, `crates/sui-id-web/src/components/chrome.rs`, `crates/sui-id-web/src/components/setup.rs`.
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

### 1 — `--fg-subtle` fails AA as text, and the gate excludes it

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

**Decision.** Placeholders get their own AA-compliant token rather than
sharing a decorative one — a placeholder is content. The remaining uses
move to a value clearing 4.5:1 in both modes. **The exclusion is then
removed from the contrast assertion**, so the contract covers what it
claims, and no token is exempt on the grounds of a claim about where it
is used.

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

### 4 — interactive controls reach 44×44

`components/buttons.rs:28` sets `min-height: 36px`. WCAG 2.5.8 (AA,
24×24) passes; **WCAG 2.5.5 (AAA, 44×44) does not**, and 44 is also both
major platform guidelines. Reduced precision and tremor are the common
case here, and a mis-tap on a destructive control costs more than a
mis-tap elsewhere.

**Decision.** 44×44 for interactive controls.

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
