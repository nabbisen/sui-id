# RFC 132 — security review, 2026-10-01

**By the architect, who authored this RFC, and therefore not independent.**
`@nabbisen` requested the audit this RFC acts on; he did not ask for a design
review of the RFC itself, and did not state that he performed one. Approved with
"Accepted." Carried under `ROADMAP.md` R1's residual, and recorded under S1c,
which rules self-review generally prohibited with an owner-invoked exception —
this is the standing R1 arrangement, not an ad-hoc request.

## What is actually at risk

This RFC touches no code. Its security content is entirely in **who is told what,
and when** — so the review asks one question: does any decision here make a
disclosure *worse*?

## Finding 1 — D1's `blank_issues_enabled: false` is the only decision that can backfire

It is also the only one that removes an option from a stranger. The failure mode
is not security but silence: someone with a report that fits none of the three
templates may abandon it rather than force it into the nearest box. A
vulnerability reporter is exactly the person most likely to have an odd-shaped
report.

**Mitigation already in the design:** the `contact_links` half of D1 closes the
security gap by itself and is independent of the blank-issue setting. If the
setting proves to turn people away, revert that one line and the gap stays closed.
The RFC says so in its Risk section rather than leaving it to be discovered.

**Not a blocker**, and I would not reverse it: for a project whose worst public
issue is a zero-day, every issue passing a surface that can carry a warning is
worth the friction.

## Finding 2 — D2 must not become a reason to withhold

Naming what to redact — cookies, bearer and refresh tokens, client secrets, key
material, real users' addresses — risks the opposite error: a reporter who
concludes the logs are too sensitive to share at all, and files a vague report
that cannot be reproduced.

**Required of the implementation:** the instruction says *redact*, not *omit*, and
says that a redacted log is wanted. Phrasing that reads as "do not paste logs"
would make bug reports worse while looking more careful. The wording should be
checked against that reading before it lands.

## Finding 3 — D3 removes a promise; it must not read as removing the credit

Aligning `SECURITY.md` with S1d means the advisory half of *"credit in the
changelog and security advisory"* goes. A reporter reading the diff could take
that as the project withdrawing recognition.

**Required:** the replacement states plainly that credit **is** given, in
`CHANGELOG.md`, and that an advisory may follow if one is issued. The thing being
removed is a promise the project would not keep — not the credit itself.

## What I checked and found no problem with

- **D0 takes nothing away and closes a real class.** No address exists in these
  files today, so D0 is preservation, not change; extending it to *not asking a
  reporter* for one is the half that was missing, and costs nothing.
- **No decision here weakens the reporting path.** D1 adds a route, D2 adds an
  instruction, D3 and D4 correct false statements, D5 adds a channel where there
  was none. The only subtraction is the blank-issue option, addressed above.
- **D6 adds no gate and no workflow**, and correctly keeps a nondeterministic job
  out of `[gates]` — a lane that can fail on a freshly generated input would fail
  for reasons unrelated to the change under test. Its reasoning is measured rather
  than asserted: the cost per case, the Argon2 exception, and the
  `PROPTEST_CASES` behaviour were each tested, and one of the three overturned the
  architect's own prior suspicion.
- **D4's RFC-lifecycle paragraph is the highest-value item and carries no risk.**
  Telling a contributor that behaviour changes go through an RFC cannot harm
  anyone; its absence has been costing contributors work that could not merge.

## Verdict

**Accept.** Three findings, none a blocker, all about wording that the
implementation must get right rather than about the decisions themselves. The
RFC's own Risk section already names the one decision that could need reverting,
and names the independent half that should survive if it does.
