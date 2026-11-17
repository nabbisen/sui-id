# Cycle N and `0.92.0` — scoped against what it would really cost

**Proposed.** 2026-11-15, by the architect, on `@nabbisen`'s *"Scope 0.92
carefully."*, after `0.91.1` was published (`d62424e`).
> ## Decided 2026-11-16 (`@nabbisen`: *"Yes to all."*)
> All three of §5, as recommended: the reordering of §2; RFC 144 with palette
> B's values; and F4's upgrade rule. The ROADMAP's rows N, O and P are
> rewritten to match. **RFC 100 stage 0 is dispatched at once**, and RFCs 147
> and 144 once Track D's two RFCs exist.

**Whose turn.** The architect's (Track D's RFCs) and the dev team's (stage 0).
**Measured on.** `bab70c9`.
**What already stands.** Cycle N is **authorized** (ROADMAP, *Release
cycles*). Its row lists:
- RFC 100, stages 0–3;
- RFC 147;
- RFC 144;
- RFC 107's last closure item;
- 096-B1 stage 14, on its prepared commit;
- RFC 097's design, *"in parallel"*.

This document checks that list against the other accepted RFCs and the code.
It finds **five things the row did not account for**, and proposes a
reordering. It is a proposal: the authorized row stands until you change it.

## 1 — what measuring the row found

| # | Found | Measured | Consequence |
|---|---|---|---|
| **F1** | **RFC 144 and RFC 149 specify different colours.** RFC 144's refresh fixed palette A's AAA values. RFC 149 then chose palette **B**, both accepted 2026-11-15 | 144: `--accent-default` `#5741AA`; 149: `#3949B2` (handoff 149, README) | Built as written, every colour changes **twice**: to A in `0.92.0`, then to B in `0.94.0`. Two visual upheavals for operators, and the AAA gate's expected values written twice |
| **F2** | **RFC 107's last item is RFC 148's work.** 107's closure is *"every subcommand … appears in `--help`; the lists cannot drift … without a test failing; a flag's value is never mistaken for a subcommand"* | `rfcs/accepted/107-cli-help-completeness.md:10`. RFC 148's single registry generates help from dispatch, and `clap` does the parsing | Fixing `setup`'s usage line by hand in N is replaced in O by generated help. **107 closes with 148**, in O |
| **F3** | **RFC 097 cannot run *"in parallel"* with N.** It needs M4-B closed, and M4-B needs stage 14, which runs at the **end** of N, on its prepared commit | ROADMAP, cycle N's row | As authorized, RFC 097 starts only after N is tagged. **The project's two long poles run one after the other**: RFC 100's build (the dev team) through N and O, then RFC 097 (the architect). That serializes what could overlap |
| **F4** | **RFC 100's stage 1 has an open design point that is yours.** An existing database has no sentinel; how one is first written is *"Proposed (the architect's, to be confirmed in this stage's dispatch)"* | `rfcs/handoffs/100-master-key-rotation/implementation.md:87-88` | Stage 1 cannot be dispatched until it is confirmed |
| **F5** | **Cycle N is large.** RFC 100's stages 0–3 are the harness (an adapter with three implementations, and materialization of simulated states), then the startup key check with its migration, then the preconditions and `--check`, then the journal. RFC 144 touches all 11 component stylesheets, 39 pages and the type scale. RFC 147 has five repairs | stage list in `implementation.md`; RFC 144 measured above | Planned as about ten dispatches. 096-B1 planned nine and took fifteen. The visible improvements, and stage 14, would wait behind all of it |

## 2 — a reordering that keeps everything and starts both long poles sooner

**The principle.** Ship first what the owner and operators feel (the
repairs and legibility), together with the one piece of RFC 100 that
changes no behaviour (its harness). Run stage 14 on that early commit, so
that M4-B closes, and **RFC 097 (the architect) then runs while RFC 100's
build (the dev team) continues**. Nothing is dropped; it is ordered so the
two people's long poles overlap.

| Release | Contents | Notes |
|---|---|---|
| **`0.92.0` (N)** | **RFC 147** (the five repairs); **RFC 144**, with **palette B's values** (F1); **RFC 100 stage 0**, the harness alone, test-only; then **stage 14** on the prepared commit | Smaller, visible, and safe: the harness changes no behaviour. **Its tag waits for stage 14's record**, as authorized |
| **`0.93.0` (O)** | **RFC 100 stages 1–4**: the startup key check (after F4's answer), the preconditions and `--check`, the journal, and recovery; **RFC 148**, which closes **RFC 107** with it | **RFC 097 drafted alongside**, by the architect, now that M4-B is closed |
| **`0.94.0` (P)** | **RFC 100 stages 5–7**, then M2c's closure review; **RFC 149**; Track D's editing a user and password policy | P carries RFC 149 as authorized. RFC 100's split moves its last stages here |

**What changes against the authorized rows:**
- RFC 100's stages 1–3 move from N to O, and 5–7 from O to P;
- RFC 107's item moves from N to O, inside RFC 148;
- RFC 144 takes palette B's values now, instead of A's now and B's later.

**What gains:**
- the UX improvements reach operators a cycle sooner;
- stage 14 runs a cycle sooner;
- RFC 097's design starts a cycle sooner, overlapping RFC 100's build rather
  than following it;
- the colours change once.

**What it costs:** RFC 100's build, and so M2c's closure, finish in P
instead of O. **M6 still waits for both M2c and M5.** M5 now starts a cycle
earlier, so if RFC 097 is the longer of the two, M6 comes sooner, and if
RFC 100 is, no later than authorized.

## 3 — `0.92.0` in detail

| # | Work | Who | Measured size |
|---|---|---|---|
| **N0** | **Done 2026-11-16**: RFC 150 and RFC 151 written, and RFC 101 refreshed, all for acceptance. **Track D's two RFCs written**: an administrator edits a user (the display name now; the email through RFC 101's lifecycle); password policy, stricter-only, never below 12. **RFC 101 refreshed** for acceptance | architect | The track's rule is that nothing in it is built until all its RFCs exist, so **N0 comes before N1–N2's dispatch** |
| **N1** | **Landed 2026-11-16** — [`0.92.0-n1-the-repairs-2026-11-16.md`](0.92.0-n1-the-repairs-2026-11-16.md). **RFC 147**: labelled primary actions (3), empty states with a next step (2), the missing-config message, the production template's keys, URL fields and the create/edit consent field | dev team | small; one dispatch |
| **N2** | **N2a landed 2026-11-16** — [`../144-legible-to-older-eyes/0.92.0-n2a-colour-2026-11-16.md`](../144-legible-to-older-eyes/0.92.0-n2a-colour-2026-11-16.md), colour and the gate; **N2b landed 2026-11-17** — [`../144-legible-to-older-eyes/0.92.0-n2b-size-and-structure-2026-11-16.md`](../144-legible-to-older-eyes/0.92.0-n2b-size-and-structure-2026-11-16.md), size and structure. **RFC 144** with palette B: the AAA gate over composed pairs (11 stylesheets); the `rem` type scale (5 tokens) and spacing; 44×44 controls; `prefers-contrast`; placeholders → hints (6); the account menu on all 39 pages; badges and denser tables | dev team | medium; probably two dispatches (tokens and gate; then the components and pages) |
| **N3** | **Landed 2026-11-16** — [`../100-master-key-rotation/0.92.0-stage-0-the-harness-2026-11-16.md`](../100-master-key-rotation/0.92.0-stage-0-the-harness-2026-11-16.md). **RFC 100 stage 0**: the key-file adapter, Real, Failpoint and Simulated; materialization; today's rotation moved onto it; the prototype's results reproduced | dev team | medium; the largest single item. It changes no behaviour |
| **P** | **Landed 2026-11-17** — [`release-0.92.0-preparation-2026-11-17.md`](release-0.92.0-preparation-2026-11-17.md). The preparation | dev team | small |
| **N2c** | **Dispatched 2026-11-17** — [`../144-legible-to-older-eyes/0.92.0-n2c-a-person-outside-the-console-2026-11-17.md`](../144-legible-to-older-eyes/0.92.0-n2c-a-person-outside-the-console-2026-11-17.md). A person outside the console sees no console navigation; found in P's review, fixed before stage 14 | dev team | small |
| **S14** | stage 14 on N2c's landing commit | owner (browser) + architect | about 15 minutes of yours |

## 4 — risks, stated

- **N3 may not finish with N1–N2.** If so, `0.92.0` ships N1–N2 without it,
  and N3 opens O. It changes no behaviour, so it has no release of its own
  to protect.
- **Palette B in N2 brings forward a piece of RFC 149.** It is only the token
  values, which RFC 149 §6 chose. The new navigation, forms and help stay in
  P.
- **RFC 144's account menu is rebuilt in P**, when RFC 149's sidebar
  replaces the top bar. It is a small cost, and the misplaced sign-out on
  38 pages is a defect worth fixing now.

## 5 — your decisions

1. **The reordering of §2**: RFC 100 split across N (stage 0), O (1–4) and P
   (5–7); RFC 107 into O; RFC 097 in O alongside. Recommended.
2. **RFC 144 built with palette B's values** in `0.92.0`, so the colours
   change once. Recommended.
3. **F4, RFC 100 stage 1's upgrade rule**, needed before O. In
   `implementation.md` stage 1:
   - if any row of the six tables K07 reseals exists, the active key must
     open one of them before the sentinel is written, or startup refuses;
   - otherwise the database has never been sealed, and the sentinel is
     written under the active key.

   Recommended. It never infers *"never sealed"* from a missing sentinel
   alone.

## 6 — not in `0.92.0`

| Item | Where |
|---|---|
| RFC 100 stages 1–7 | O and P (§2) |
| RFC 148, and RFC 107's closure with it | O |
| RFC 149, and Track D's two features | P |
| Adding a federation provider from the console | after M7, as decided |
| RFC 145 (passkey alone), RFC 146 (TOTP proof) | unchanged: a freeze exception, and M7 |
