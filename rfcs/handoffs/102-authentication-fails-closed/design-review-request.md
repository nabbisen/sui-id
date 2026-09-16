# RFCs 102 and 103: independent design review request

**RFCs.**
- [RFC 102](../../accepted/102-authentication-fails-closed-without-audit.md) —
  authentication that cannot be audited does not succeed. Part A covers sign-in;
  Part B covers step-up.
- [RFC 103](../../accepted/103-administrator-issued-account-recovery.md) —
  administrator-issued account recovery.

Both are Proposed.

**Reviewer.** Mid-capability model, implementation role. It authored neither RFC.

**Baseline.** The commit that adds RFC 103, or later. *Revised 2026-09-16:* the
first version of this request covered RFC 102 Part A only, at `8fc0c62`. If you
started from that version, keep what you have and add the new items below.

**Scope.** Read-only. Change no code and no RFC text. Report findings. The two
RFCs are reviewed together because RFC 103 depends on RFC 102 Part B.

## RFC 102 Part A — sign-in

1. **Is the path inventory complete?** Find every production call that inserts a
   session row: `sessions::insert`, `insert_within_tx`,
   `commands::insert_session`, or raw SQL into `sessions`. Part A lists five
   paths; a sixth would be a blocker. Give `file:line` for each call site.
2. **Does each claim in Part A's background hold at the baseline?** One row per
   checkable sentence: the claim, `file:line`, and whether it holds.
3. **The TOTP replay race.** Confirm or refute it by reading. If you can, write a
   test that demonstrates it. Put the test in the review package only; do not
   commit it.
4. **Can L01–L04 be built on the existing runner?**
   - Can each transaction's contents run on `Database::class_a` with the existing
     `*_within_tx` functions?
   - Name every function that would need a `_within_tx` variant.
   - Can `enforce_concurrent_session_cap`'s `server_settings` read run inside the
     transaction?
5. **A3, uniform failure.** For each path, state the response a failed
   transaction would produce under the design. Is it byte-identical to that
   step's ordinary failure?
6. **A4, no bypass.** Can `sessions::insert` become crate-private without
   breaking a legitimate caller that is not a sign-in, tests included?

## RFC 102 Part B — step-up

7. **Do B-F1 to B-F6 hold at the baseline?** One claim-table row each. In
   particular:
   - Is there really no rate limit or failure count on `POST /me/security/step-up`?
   - Does `verify_totp_code` accept and consume a recovery code?
8. **Gated call sites.** List every `require_fresh_step_up` call site, with the
   Class-A command each gated handler then runs. B4 adds a required attribute to
   exactly those commands' descriptors. Name any gated handler that runs no
   Class-A command, or more than one.
9. **Can L05 and L06 be built?**
   - The `sessions` migration.
   - `finish_authentication` taking the expected ceremony kind.
   - How B4's evidence value gets from the gate to the command without handlers
     being able to forge it.
10. **Open question 3.** Which self-service MFA actions are step-up-gated? If
    recovery-code sign-in grants no freshness, can a user who has lost their only
    authenticator still recover? Can a sole administrator? Trace both through
    the code.

## RFC 103 — recovery link

11. **Every credential writer.** List every production `credentials::upsert` and
    `upsert_within_tx` call site, with the command or flow it belongs to. RFC 103
    says only U01, first setup, U09 and U10 remain after U06 is retired.
12. **Reuse of the forgot-password machinery.** Can `password_reset_tokens` and
    U10 serve admin-issued links with only the D2 columns added? Name anything
    in `forgot_password.rs` that assumes an email origin.
13. **D3 invalidations.** For each of password change, user disable and user
    delete: are outstanding tokens invalidated today? Give `file:line`.
14. **Is U06 really unreachable?** Name every caller of the core function and of
    the store command, tests included, so its retirement has a complete list.
15. **Threats.** Name any threat in RFC 103's table whose control would not stop
    it as designed, and any threat the table misses.

## Both RFCs

16. **Open questions.** Give your view on each open question in both RFCs, with
    reasons, or say it is outside what this role can adjudicate.

## What to return

A review-request package under `.git-exclude/review-requests/`, containing:
- findings, ranked by severity (blocker, high, medium, low), each tagged with its
  RFC;
- the claim tables for items 2 and 7;
- the call-site lists for items 1, 8, 11 and 14.

---

## Confirmation review of the resolutions — requested 2026-09-16

The review returned one blocker, six high, eight medium and six low findings.
`323c90a` resolves them in the RFC text. RFC 102 carries a resolution table;
RFC 103 names each finding (H5, H6, M5, M7, M8) where it is resolved. RFC 000 accepts a design only after its reviewed form is the accepted
form, so confirm the resolutions before either RFC goes to the owner.

**Baseline.** `323c90a` or later. **Scope.** Read-only, as before.

17. **For each finding in your review, one row:** does the cited section resolve
    it, partly resolve it, or leave it open? Quote the resolving sentence. For
    the three live defects (H3, H4, H5), confirm the RFC records them rather than
    claiming to fix them now.
18. **New design introduced by the resolutions.** Check each for implementability
    and for new gaps:
    - RFC 102 B7's first-factor rule (password re-entry for local users; session
      age for users without a local password);
    - B4's command-side evidence (the session ID as a command input);
    - L07;
    - A7;
    - RFC 103 D10's fragment link, including the no-JavaScript fallback;
    - D13's in-transaction `source` and state re-read;
    - D3's invalidations in U02, U04 and U09.
19. **The failure-response table** in RFC 102: does each row match what the
    handler can return after the change, without leaking the cause?

**Return.** A short review-request package: the row table, and findings ranked
by severity. Findings on the new design only; do not re-open what the first
review settled.
