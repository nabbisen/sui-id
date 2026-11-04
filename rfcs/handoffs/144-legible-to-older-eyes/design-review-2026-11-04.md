# RFC 144 — the design review record

**What this file is.** The durable record RFC 144's
`Independent design review` field cites. Per `rfcs/README.md`'s errata
on that field, it says what the review was and was **not** independent
of.

## Who reviewed it, and under which rule

**The architect, at `@nabbisen`'s request.** His words, 2026-11-04:
*"Reviewed. Almost accepted. Review it carefully by yourself with my
fundamental philosophy."* — `ROADMAP.md` §S1c's owner-invoked exception
to the self-review prohibition, which requires the record to say he
asked. **He asked.** He also reviewed it himself first; this record does
not speak for that review and contains none of his findings, only mine.

**The architect wrote RFC 144 and is therefore not independent of it.**
§S1a makes engaging an external architect his option; none was engaged.
The review below finds that **the RFC's central diagnosis is too
narrow** and that its closure prerequisite is unverifiable as written.

---

## 1 — the finding: the gate's *scope* is the defect, not its exclusion

RFC 144 §1 says the contrast gate reports green because it **excludes**
`--fg-subtle` on a false premise. True, and insufficient.

**`crates/sui-id-web/src/tokens/tests.rs` reads `TOKENS_CSS` and
nothing else** (`:19`, `:97`). It never sees `CONFIRM_CSS`,
`BANNERS_CSS`, or any other component stylesheet. So the contract
covers the **palette** — pairs the token file itself declares — and not
the **compositions** components create by putting one token's colour on
another token's background.

A component may pair any two tokens, and nothing checks it. Removing the
`fg-subtle` exclusion fixes a symptom and leaves the mechanism intact.

## 2 — three AA failures the RFC missed, all in destructive-action UI

Found by measuring the pairs components actually compose. All in
`crates/sui-id-web/src/components/confirm.rs`:

| element | pair | measured | size | AA |
|---|---|---|---|---|
| `.reversibility-badge--permanent` (`:31-32`) | `--danger-default` on `--danger-subtle` | **4.28:1** | 13px | **FAIL** |
| `.reversibility-badge--recoverable` (`:27-28`) | `--success-default` on 15% success over surface | **3.80:1** | 13px | **FAIL** |
| `.danger-zone__title` (`:48-51`) | `--danger-default` on `.danger-zone`'s `--danger-subtle` | **4.28:1** | 15px | **FAIL** |

**What these elements are matters more than the numbers.** The
permanence badge is the label telling an operator *this action cannot be
undone* (RFC 017 §3). The danger-zone title heads the section that
*"physically and semantically isolates destructive operations"* — its
own comment. So the two places in this interface that exist to stop
someone destroying something by accident are rendered below the contrast
standard, one of them at caption size.

Against the second half of the philosophy — *APIs and UI/UX for users
not to be confused or misunderstand* — this is the worst-placed defect
in the product. It is not a muted footnote; it is the warning.

**`.danger-zone__body` is correct** (`--fg-default`, 13.72:1), which
makes the title's failure a composition slip rather than a palette
problem.

## 3 — and the banners are right, which the fix must not disturb

`components/banners.rs` pairs every tinted background with
`color: var(--fg-default)` — **13.72:1 to 15.06:1**. `.flash.warn`,
`.flash.error`, `.banner--success`, `.banner--warning`, `.banner--danger`
all do this.

So the codebase already contains the correct pattern: **tint the
background, keep the text at full foreground contrast.** The semantic
colour belongs on the border, which `banners.rs` also does. The confirm
screens departed from a pattern the rest of the UI follows. The fix is
to apply the existing convention, not to invent a palette.

This also means the "message areas" `@nabbisen` asked about are **not**
where the problem is — the labels are.

## 4 — the RFC's closure prerequisite cannot be discharged as written

It says: *"Every token used as `color:` clears WCAG 2.1 AA (4.5:1) in
both modes."*

**A token has no contrast ratio. A pair does.** `--danger-default`
clears AA on white (5.29:1) and fails on `--danger-subtle` (4.28:1) —
the same token, both facts true. As written the prerequisite is
satisfiable by checking each token against the default surface, which is
precisely the check that missed §2's three failures.

**This is a defect in my own RFC, of the kind this project keeps
finding**: a criterion phrased so that the easy reading passes.

## 5 — why I missed it, stated plainly

I enumerated the **palette** and the on-colour pairs the token file
declares, then asked which tokens are used as `color:`. I never asked
what each component puts *behind* that colour. The population I sampled
was tokens; the population that matters is composed pairs.

That is the same error as pricing an alternative on the wrong axis, and
it is the second time this week that measuring the thing in front of me
rather than the thing that matters has produced a confident, narrow
answer.

## 6 — two further corrections to the RFC

**§4's 44×44 is too broad as written.** WCAG 2.5.8 exempts inline links
in prose from target sizing; a blanket *"44×44 for interactive
controls"* would wrongly inflate them. Scope it to standalone controls —
buttons, form controls, menu items — and say so.

**§3's `rem` conversion is incomplete.** The type scale would move to
`rem` while the spacing scale stays `px` (`--space-1: 4px` through
`--space-6: 48px`). At a reader's enlarged font size, text grows and
padding does not: lines crowd their containers, and the accommodation
partly defeats itself. Either convert spacing with it or state why not.

## 7 — one question left open, not a decision

RFC 144 fixes the placeholder by giving it an AA-compliant token.
Necessary, and possibly not the right shape. Placeholder text
disappears the moment typing begins, which is when an uncertain reader
most wants it. `pages/confirm.rs:143` puts a placeholder on a textarea
**on a confirmation screen**.

Darkening it is strictly an improvement. Not relying on it for guidance
that matters is a product decision, and his.

## 8 — recommendation

**Do not accept RFC 144 as written.** §1's diagnosis should name the
gate's scope rather than its exclusion; §2's three failures and the
`banners.rs` convention belong in the design; the closure prerequisite
must be phrased over **pairs as components compose them**, which is a
larger gate change than the RFC currently proposes; and §§3–4 need the
two corrections above.

The RFC is amended accordingly in the same change as this record, and
`@nabbisen`'s *"Almost accepted"* is left as exactly that until he reads
the amendment.
