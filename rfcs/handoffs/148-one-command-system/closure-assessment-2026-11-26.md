# RFC 148 and RFC 107 — closure assessment

**Written.** 2026-11-26, by the architect, at `74ff966`. Cycle P's item (ROADMAP, cycle P's row).
> **RFC 148: decided 2026-11-26.** `@nabbisen`: *"Your recommendation is accepted."* That is option 1: the pty test first,
> as cycle Q's first item, then RFC 148 closes at `0.95.0`.

**Not independent.** The architect wrote both RFCs' dispatches and reviewed their returns. It recommends; it decides nothing.
**Published** after `0.94.0` was tagged, so that `main` did not move under it.

## RFC 107 — every subcommand in `--help`, and the lists cannot drift

Its closure prerequisites, each against what proves it:

| Prerequisite | Evidence | Shipped |
|---|---|---|
| Every subcommand the binary dispatches appears in `--help` | `cli::help::tests::overview_fits_one_screen_and_names_every_leaf`; `every_leaf_has_notes` | `ff1a9f5`, in `0.93.0` |
| The lists cannot drift again without a test failing | RFC 148's one registry generates dispatch, help and the reference; `cli::reference::tests::the_committed_page_matches_a_fresh_render` fails on any divergence | `0.93.0` |
| A flag's value is never mistaken for a subcommand | `cli::tests::a_flags_value_is_never_taken_for_a_command`; `a_value_that_spells_a_command_name_is_never_mistaken_for_one` | `ff1a9f5`, in `0.93.0` |

**Closed as Implemented (v0.93.0)**, `@nabbisen`, 2026-11-26: *"Close RFC 107 (Recommended)"*. Every prerequisite is met by tests that shipped in `0.93.0`.
RFC 148 is the mechanism that keeps them met. Its mutations were run in O2a's review.

## RFC 148 — one command system

| Prerequisite | Evidence | Met? |
|---|---|---|
| Every command is declared once, in one registry, with dispatch, help and reference generated from it, and a test fails on divergence | The registry (O2a); `e2e::o2a_cli_registry::help_and_the_registry_cannot_diverge` (through the binary); `the_committed_page_matches_a_fresh_render`; the help tests | **Yes** |
| Every pre-148 form still works and prints a one-line notice naming its new form | five `e2e::o2a_cli_registry` tests through the binary, one per kind of old form (for example `an_old_admin_subaction_alias_prints_notice_and_has_the_same_effect_as_its_new_form`), and `bare_config_serves_and_prints_no_notice` for the new form; `cli::tests::every_old_admin_subaction_word_parses` | **Yes** |
| `sui-id init` never overwrites a file | `e2e::o2b_init_and_first_run::init_never_overwrites` (through the binary; O2b's mutation was `create_new` → `create`) | **Yes** |
| With no configuration: **without a terminal**, prints the guidance and exits non-zero, never waiting | `no_terminal_no_prompt_exits_one_promptly_with_the_guidance`; `explicit_missing_config_never_prompts_even_with_stdin_open` | **Yes** |
| With no configuration: **in a terminal, offers to create one** | `cli::first_run::tests::decide_covers_all_eight_cases`, a pure function. Plus **the architect drove the binary by hand through a real pseudo-terminal** in O2b's review, answering `n`, `x`, end of input and `l` | **Not as the RFC requires** |
| Each of these tested **through the binary, and mutated** | as above, except the terminal case | **Not for the terminal case** |

**The one gap, stated exactly.**
- The terminal case was proven once, by hand, through a pty, and its decision is tested as a pure function.
- **No automated test runs the binary in a terminal.** A change in how the binary detects a terminal (`stdin().is_terminal()
  && stderr().is_terminal()`, `first_run.rs`), or in what it does once it does, would pass every test.
- O2b's review accepted the hand drive because there was no pty crate, and adding a dependency for one test was not worth
  it. That reasoning still holds for a crate. **It does not hold for the test**: util-linux's `script` gives a child a real
  terminal with no Rust dependency (`script -q -c "<cmd>" /dev/null`, stdin piped to it). It is here (util-linux 2.42.4), and
  it is part of a standard Ubuntu image. CI's runner is the confirmation.

**Two ways to close it, for the owner:**
1. **Recommended: one small test first, then close RFC 148** at `0.95.0`. Cycle Q's first item: an e2e test that runs
   `sui-id` with no configuration under `script`.
   - It answers `n` and asserts the offer appears and nothing is written.
   - It answers `y`, or the template key, and asserts the file is created with `init`'s content.
   - **Mutation:** `decide` ignoring `is_terminal`, or `is_terminal()` forced false, and the test fails.

   If `script` is missing on the runner, the test fails loudly. It does not skip.
2. **Close RFC 148 now**, on a ruling that the hand drive plus the pure-function test meet *"tested through the binary"* for
   this one case. That is an exception to the RFC's own words, so it would be yours to make and would be recorded as such.

**The architect recommends 1.** A hand drive proves the property once. A test proves it on every change, which is the
difference half 1 asks for.
