# RFC 139 — the newcomer's path, and an independent review RFC 107 is missing

**Dispatched.** 2026-10-14 JST, by the architect.
**RFC.** 139 — Getting Started leaves its warnings in other documents.
**Status: Accepted** 2026-10-14, documentation only.
**Baseline.** Read the tip with `git log -1` and name the full SHA.
**Also in scope.** One independent design review of RFC 107 — see §6. That is a
document, not code.

Read RFC 139 first; it has the measurements and the decided wording. This
dispatch says what to build and what will be checked.

## 1 — D4 first, because it is the gap

The other four are corrections. This one is the thing a newcomer came for and
the documentation does not have: `grep` across all of `docs/src/` and
`README.md` for registering a first client returns **nothing**. The Quick Start
ends at a running server and a table of endpoint URLs.

Write the short worked example that ends the page: register one client in the
admin panel, and the handful of values an application needs from it. Do not
duplicate [the OIDC API reference](../../../docs/src/reference/oidc-api.md) —
get the reader to a first success and link there for the rest.

**Verify it by doing it**, the way the review verified the rest of the page: run
the server, follow your own text, and say in the package that you did. If a
step you wrote does not work, that is the finding.

## 2 — D1, the maturity statement: one source, two pointers

RFC 139 carries the exact replacement text for `faq.md`'s answer. Use it as
written; it was decided against the ROADMAP's own wording and approved.

`README.md` under the tagline, and `quick-start.md` at the top, each get **one
line** — pre-1.0, under active development, not yet recommended for production
— linking to that answer. **One line each, not a paragraph.** The point of the
pointers carrying no detail is that they cannot drift; a pointer that restates
the statement recreates the defect this decision exists to remove.

Leave `quick-start.md:94`'s "Never use `--dev` in production" alone. It is true.

## 3 — D2, prerequisites against both install commands

`cargo install sui-id` compiles from source, so it needs what
`deployment.md:77` names — `build-essential pkg-config libssl-dev` — and the
binary then needs `libssl.so.3` (`deployment.md:36`, `:45`, `:99`). The MSRV
caveat at `README.md:81-83` is currently attached only to "build from source"
while `cargo install` is listed first and hits the same wall.

State both against both commands, or once in a line that plainly covers both.
Do not copy the Deployment guide's apt line into the Quick Start — name what is
needed and link.

## 4 — D3, stop sending a first-time reader into a permission error

`quick-start.md:27-32` tells the reader to set
`db_path = "/var/lib/sui-id/sui-id.sqlite"` and
`key_file = "/etc/sui-id/sui-id.key"`. The sample config's own defaults are
`./sui-id.sqlite` and `./sui-id.key` and they work. Drop the system paths from
the Quick Start and link to the Deployment guide, which already has them in the
context where they are right.

## 5 — D5, the two screenshots

Already taken and committed at `docs/assets/screenshot-setup.png` (the first-run
wizard's welcome step) and `docs/assets/screenshot-admin-login.png` (the admin
sign-in page). Place them where they help a newcomer decide and orient —
RFC 139 does not dictate where; argue your placement in the package.

**Do not take new ones without reading RFC 139's D5.** It records how these were
produced so the owner's privacy is protected by method rather than by luck:
headless `chromium` with `--user-data-dir` pointed at a throwaway directory, no
browser chrome in the image, throwaway data, and a human look at the image
before it is committed. Any future screenshot is produced and checked the same
way.

## 6 — the independent design review RFC 107 needs

RFC 107 is `Proposed` and **cannot be accepted as it stands**. Measured: adding
the acceptance metadata and running G11 gives

```
check-rfc-integrity: rfcs/accepted/107-cli-help-completeness.md:
  Security review is Required but 'Independent design review.' is missing
```

The architect authored RFC 107, so the architect cannot supply that review. You
did not author it, so you can.

Review RFC 107 as a design — is the problem real, is the fix the right shape,
does its closure prerequisite ("a flag's value is never mistaken for a
subcommand") actually follow from what it proposes — and write the result to
`rfcs/handoffs/107-cli-help-completeness/design-review-2026-10-14.md`. **Say
what is wrong with it if something is.** A review that finds nothing is a
result, but it is the less likely one.

Do **not** implement RFC 107. It is not ours to accept; that comes after the
review.

## What this stage must not do

**No code.** RFC 139 touches `README.md`, `docs/src/getting-started/`, and
`docs/assets/`. `crates/` is not in scope, and the `--help`/`find_subcommand`
defect is RFC 107's, not this one's.

**No restructuring.** The nav order stays as it is.

**No new claim about readiness.** D1 records what `ROADMAP.md` already says, in
one place instead of two that disagree. Do not soften it and do not harden it.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **D4's example, and the statement that you ran it yourself**, with what you
   observed.
2. **The four corrections**, each with the before/after and the file:line it
   addresses.
3. **Your placement argument for the two screenshots.**
4. **The RFC 107 design review**, as its own file at the path above.
5. **Per-hunk SHA-256** against the tip you named — `python3.14
   scripts/hunk-hashes.py --baseline <sha>`. Full content for new files. The
   two `.png` files are binary; the tool handles them, do not hand-roll it.
6. **Gate evidence.** This is a docs change, so CI will path-filter to Level A;
   run the full matrix locally anyway and say so. **G10a (mdBook) and G10b/G14
   (links) are the ones a docs change moves**, and G15 (documentation
   authority) is the one that cares whether two documents disagree.
7. **Anything you think is wrong with this dispatch**, including the decided
   wording in D1. Three of the last four rounds found something; the rate is
   the useful number.
