# RFCs 148, 149 and 151: closure at `0.95.0`

**Written.** 2026-11-28, by the architect, at `93cc6a5` (tag `0.95.0`).
**Authority.** The cycle Q plan, as authorized on 2026-11-26 at `f0107e1` (`@nabbisen`: *"Authorized."*), closes all three at
`0.95.0`'s cut:
- RFC 148 moves to `done/`;
- RFC 151 moves once Q2 has landed;
- RFC 149 moves once Q2, Q3 and Q4 have landed.

RFC 148's closure path was also decided on its own assessment
([`closure-assessment-2026-11-26.md`](../148-one-command-system/closure-assessment-2026-11-26.md)), on 2026-11-26
(`@nabbisen`: *"Your recommendation is accepted."*): one test through a real terminal first, then close at `0.95.0`.

**Not independent.** The architect wrote every stage's dispatch and reviewed every return. This record measures each
condition against the test that holds it. It decides nothing beyond what the plan authorized.

## RFC 148: one command system

The 2026-11-26 assessment found every condition met except one: the terminal case. Q1 (`7e2c3df`) closes it.

| Condition | Evidence | Met? |
|---|---|---|
| One registry; dispatch, help and reference generated from it; a test fails on divergence | `e2e::o2a_cli_registry::help_and_the_registry_cannot_diverge`; `cli::reference::tests::the_committed_page_matches_a_fresh_render` | Yes (`0.93.0`) |
| Every pre-148 form still works, with a one-line notice | the five `e2e::o2a_cli_registry` old-form tests; `bare_config_serves_and_prints_no_notice` | Yes (`0.93.0`) |
| `sui-id init` never overwrites a file | `e2e::o2b_init_and_first_run::init_never_overwrites` | Yes (`0.93.0`) |
| Without a terminal: guidance, non-zero, no waiting | `no_terminal_no_prompt_exits_one_promptly_with_the_guidance`; `explicit_missing_config_never_prompts_even_with_stdin_open` | Yes (`0.93.0`) |
| **In a terminal: offers to create one** | five tests through the binary under util-linux `script`: `the_offer_appears_and_n_writes_nothing`, `l_writes_the_local_trial_and_does_not_serve`, `p_writes_the_production_template`, `an_unexpected_answer_is_not_now`, `end_of_input_with_no_answer_is_not_now` | **Yes** (`7e2c3df`) |
| Each tested through the binary, and mutated | Q1's review: with the terminal check forced to `false`, **all five fail**. Two did not fail on the return as first sent, and were fixed in the review. CI confirmed on `ubuntu-24.04` (run `45806896282`) | **Yes** |

## RFC 151: the minimum can be raised, never lowered

Built in `0.94.0` P5 (`a3e8dfc`). It stayed Accepted only because its condition says *"from the *Sign-in* settings"*, a
tab RFC 149 created.

| Condition | Evidence | Met? |
|---|---|---|
| Raised from the *Sign-in* settings | `e2e::password_min_len::an_administrator_raises_the_minimum_from_the_settings_tab`; `e2e::q2_navigation::rfc151_closure_the_minimum_length_is_under_sign_in` (the tab is marked current under the Sign-in group) | **Yes** (Q2, `57c80d1`) |
| Never lower than 12; a test fails if any path enforces a lower one | `s11_refuses_11_and_129_and_accepts_12_and_128`; `posting_11_names_the_floor_and_changes_nothing`; `no_path_enforces_a_fixed_minimum` (a source scan that fails on a call site it has not seen) | Yes (`0.94.0`) |
| One Class-A command, its audit record naming old and new | `s11_change_password_min_len_appends_exactly_one_audit_row`; `s11_injected_failure_before_append_rolls_back_the_change` | Yes (`0.94.0`) |
| Every page asking for a new password states the current minimum | `the_reset_page_hint_states_the_raised_minimum`; `a_raised_minimum_refuses_a_short_reset_then_accepts_the_new_length`; `a_raised_minimum_refuses_a_short_self_service_change_then_accepts_the_new_length`; `setup_refuses_a_short_first_password_and_says_why`; `the_reset_refusal_on_a_japanese_page_is_in_japanese_and_names_16` | Yes (`0.94.0`) |
| Each tested, with a mutation | P5's dispatch §4 names one per behaviour; its review: *each named mutation was run and caught* | Yes |

## RFC 149: the console is organised around what an operator does

| Condition | Evidence | Mutation, caught | Met? |
|---|---|---|---|
| Every admin page belongs to exactly one navigation group of §1 | `e2e::q2_navigation::every_admin_get_route_belongs_to_exactly_one_group`: every `GET /admin/*` route from the route table itself, 33 console pages each with one row, and 5 that render no console page, each asserted; `posts_that_render_inside_the_console`; `the_sidebar_is_the_list` | a page's item changed; a row deleted from the table; two items marked current; `NavItem::ALL` shrunk | Yes (Q2, `57c80d1`) |
| §2's first-run path reachable from the dashboard | `e2e::q3_help_and_dashboard::the_first_run_path_opens_with_recommended_and_optional_steps_and_folds`; `auditor_sees_no_path_but_still_sees_the_smtp_prompt` | step 4's *Recommended* badge removed; the SMTP item's suppression removed | Yes (Q3, `420001b`) |
| §3's help policy on every page; no hover-only tooltip | `every_console_and_account_page_has_a_nonempty_lede`; `no_hover_only_tooltip_anywhere`; `e2e::q2_navigation::chrome_has_no_title_attribute_and_no_menu_role` | a lede emptied; `title="Copy"` put back | Yes (Q3) |
| §4's list editors and scope selection work with JavaScript off | `e2e::q4_app_form::add_another_and_remove_save_nothing_and_record_nothing`; `each_redirect_uri_is_checked_on_its_own`; `the_scope_checkboxes_are_the_discovery_list_in_order`; `openid_is_always_saved_even_when_omitted` (plain form posts, no script) | the `action != Save` guard bypassed; the default rendering `openid` twice | Yes (Q4, `14a0422`) |
| Each asserted by a test over the rendered pages, and mutated | as above: every test drives the router and reads the HTML | as above | Yes |

**Beyond the conditions, in RFC 149's scope**: Q5 (`38f99f0`), §5's lists. Every list states its count and its page, and
none truncates silently.

## Outcome

All three move to `rfcs/done/` as **Implemented (v0.95.0)**, under the plan's authorization. RFC 148 and 151 shipped most
of their behaviour earlier. They are recorded as implemented in `0.95.0`, the release in which their last condition was met.
