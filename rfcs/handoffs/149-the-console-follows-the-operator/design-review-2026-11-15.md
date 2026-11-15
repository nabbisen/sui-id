# RFC 149 — the design review record, 2026-11-15

**RFC.** [`../../accepted/149-the-console-follows-the-operator.md`](../../accepted/149-the-console-follows-the-operator.md)
**Reviewer.** **The architect, at `@nabbisen`'s request**: *"Review them carefully
by yourself with my fundamental philosophy."*, on the RFC's three mockup
questions. That is `ROADMAP.md` §S1c's owner-invoked exception, as for
RFC 144 on 2026-11-04. **The architect wrote this RFC and its mockups, and
is not independent of them.** This record says so, and measures where it
can, rather than argue.
**What was reviewed.** RFC 149 at `54dccab`, its three mockups, and the
code each question touches.
**Outcome.** The three questions answered below. **The RFC itself is not
accepted by this record**, and remains Proposed.

The philosophy has two halves, and both are applied to each question:
clean, safe, robust and sophisticated; **and** nothing a user can
misunderstand.

## Question 1 — sidebar or top bar: **the grouped sidebar**

- **Nothing to misunderstand: everything is visible at once.** The sidebar
  shows all six groups and every destination, with their labels, and the
  current page marked. The top bar shows six words, and every destination
  is behind one of them. A beginner, or a reader who has forgotten where a
  setting lives, finds it by reading, not by opening menus to look.
- **Robust without JavaScript.** Each top-bar menu is a `<details>`. Current
  browsers can keep only one open at a time (`<details name>`). But none
  can close it when the reader clicks elsewhere, which needs a script. An
  open menu also covers the page beneath it. The sidebar has nothing to
  close.
- **The account menu stays apart in both.** That does not separate them.
- **The cost:** about 248px of width on a desktop. On a narrow screen the
  sidebar folds into one menu button, which opens the same list. The
  widest table, the audit log, keeps its five columns at 1280px.

## Question 2 — palette A or B: **B, with one value raised**

| Measured | A (refined current) | B (calm slate) |
|---|---|---|
| Closest pair of semantic colours, light | ΔE 0.119 | ΔE 0.120 |
| Closest pair of semantic colours, dark | ΔE **0.082** | ΔE **0.109** |
| Tint of the page greys (OKLCH chroma) | up to **0.0285** (dark): visibly purple | up to **0.0168**: near neutral |
| Warning as a border, light | 4.91:1 | 3.54:1 → **raised to 4.52:1** (`#9D6600`, hue kept) |

- **Nothing to misunderstand.** In dark mode, A's purple accent and blue
  info sit closest of any pair, in either palette. B separates them by a
  third again. And B's near-neutral greys leave colour to mean something:
  a status is not competing with a tinted background for the eye.
- **Sophisticated.** Opinion, stated as opinion. B's neutral chrome is
  calmer, and it is what the owner's *"the color palette is
  unsophisticated"* most plainly asks to move away from. A keeps sui-id's
  purple identity, from mockup `v0.4.8`. That continuity has value, but
  no one has stated it as a requirement.
- **The one change to B:** its light-mode warning, at 3.54:1, passed RFC
  144's 3:1 floor with little margin. It is raised to `#9D6600` (4.52:1).
  Its closest pair is unchanged at 0.120. The mockup is updated to show
  it.

## Question 3 — additional scopes: **keep them**, guarded

Measured, not assumed:
- A scope the client is allowed, and requests, passes
  `enforce_scope_policy` (`crates/sui-id-core/src/oidc/authorize.rs:309-327`).
- It is then written into the **access token's `scope` claim**
  (`crates/sui-id-core/src/oidc/tokens.rs:38`, `:146`), a JWT that a
  resource server can check.

So a custom scope does real work today: it is how an operator's own API
can authorize. Removing the field would break that silently. RFC 149 §4
already places it in layer 2, and refuses a near-miss of a standard scope
(*"profle"*). That is kept as designed.

## What this record changes

- RFC 149 §8 records these three answers.
- `mockups/palettes.html` shows B's raised warning value.

**It does not accept the RFC.** The RFC was accepted the same day, on this review (`@nabbisen`: *"All accepted and authorized."*).
