# RFC 139 — Getting Started leaves its warnings in other documents

**Status.** Accepted
**Accepted on.** 2026-10-14
**Approved by.** `@nabbisen`, 2026-10-14: *"Your proposal is approved."* — on the
architect's review of Installation and Quick Start against both halves of his
fundamental philosophy, which he asked for and whose findings are this RFC's
body. He had already called the findings good and said they should be fixed; the
approval above is of the proposal to carry them as one small RFC. The architect
wrote both the review and this RFC and is therefore not independent of it;
`@nabbisen` is the approver, which is what RFC 000 requires when no independent
role exists.
**Security review.** Not required — documentation only. No code, no schema, no
crate's public surface, no runtime behaviour. The one code defect the review
found belongs to RFC 107 and is explicitly out of scope below. Reason subject to
acceptance.

**Design prerequisites.** None.
**Implementation prerequisites.** This RFC Accepted — **satisfied 2026-10-14**.
**Closure prerequisites.** A reader who starts at the README or at Quick start
meets the maturity statement before the install command; the statement exists in
exactly one place and the others link to it; both Quick Start commands carry the
prerequisites they actually need; no instruction tells a first-time reader to
write to a path they do not own; and the page ends at a connected application
rather than at a list of endpoint URLs.
**Tracks.** Usability and operator trust.
**Touches.** `README.md`, `docs/src/getting-started/quick-start.md`,
`docs/src/getting-started/faq.md`, `docs/assets/`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/139-getting-started-leaves-its-warnings-elsewhere/README.md`](../handoffs/139-getting-started-leaves-its-warnings-elsewhere/README.md)

## Summary

Every command in the Quick Start works. The architect generated a config, ran
`setup`, started the server and got `200` from `/admin/login` and from
`/.well-known/openid-configuration` before writing this. The page is accurate.

What it is not is kind. Four of the five things a newcomer needs are in a
different document, and one instruction is wrong. This RFC moves those four onto
the path the newcomer actually walks and corrects the fifth. It changes no code.

## Why this is worth an RFC rather than a quiet fix

Because one of the five is a claim about whether this software is safe to run,
and the project's two answers to that question disagree. That is not a typo to
be swept in with a docs pass; it is the single most consequential sentence a
self-hosted IdP shows a stranger, and it should change on the record, with
`@nabbisen`'s name on it.

The other four ride along because they are the same journey and splitting them
would cost more in process than in writing.

## D1 — One maturity statement, in one place, that the reader meets first

`faq.md` answers "Is sui-id production-ready?" with "under active development…
run it in non-critical environments first." `ROADMAP.md:13-18` says the tree is
"**not approved for a production release, a 'security-reviewed' claim, or a
v1/rc/beta designation**." Those are different claims. The FAQ is the third
page; the path is Overview → Quick start → install. The only production sentence
a Quick Start reader meets is `quick-start.md:94`'s "**Never use `--dev` in
production**", which implies the normal path is production-blessed.

The FAQ answer becomes the single source and is strengthened to the ROADMAP's
substance:

> sui-id is pre-1.0 and under active development. It has not completed its
> security-assurance programme, so the project makes no "security-reviewed"
> claim and no compatibility guarantee on any interface — not the HTTP API, the
> admin UI, the CLI or the configuration file. Run it in non-critical
> environments, and read the CHANGELOG before upgrading. The programme that must
> finish before any production-readiness claim is in `ROADMAP.md`.

`README.md` (under the tagline) and `quick-start.md` (at the top) each carry
**one line** saying pre-1.0, under active development, not yet recommended for
production, linking to that answer. **One source, two pointers, deliberately.**
Four copies is how the README and the FAQ drifted apart in the first place, and
a pointer carrying no detail cannot drift.

`quick-start.md:94` keeps "Never use `--dev` in production" — it is true — but
stops being the page's only word on the subject.

## D2 — Both install commands carry the prerequisites they need

`cargo install sui-id` compiles from source. `deployment.md:77` says that needs
`build-essential pkg-config libssl-dev`, and `:36`, `:45` and `:99` say the
binary needs `libssl.so.3` at runtime. Neither Quick Start says so, and a
newcomer has no reason to open the Deployment guide before installing.

`README.md:81-83` does warn that MSRV 1.95 is newer than distribution
toolchains and that rustup is wanted — but only under "build from source".
`cargo install` compiles too and is listed **first**, so the caveat is attached
to the wrong command.

Both Quick Starts state the build prerequisites and the MSRV caveat against
**both** commands, or once in a line that plainly covers both.

## D3 — No instruction that sends a first-time reader into a permission error

`quick-start.md:27-32` says "Edit at minimum" and shows
`db_path = "/var/lib/sui-id/sui-id.sqlite"` and
`key_file = "/etc/sui-id/sui-id.key"`. A newcomer running as themselves can
write neither. The sample config's **own defaults** are `./sui-id.sqlite` and
`./sui-id.key` — measured from real `--print-sample-config` output — so the page
replaces something that works with something that fails.

The Quick Start stops proposing system paths. They are correct for a deployment
and the Deployment guide already has them; it links there instead.

## D4 — The page ends at a connected application

`grep` across all of `docs/src/` and `README.md` for registering a first client
returns nothing. The Quick Start ends at a running server, the admin panel and a
table of endpoint URLs. A newcomer installs an OIDC provider to connect an
application; no document walks them from "setup complete" to "my application
logs in".

The Quick Start gains a short worked example: register one client in the admin
panel, and the handful of values an application needs from it. It does not
duplicate the OIDC API reference — it gets the reader to a first success and
links there for the rest. **This is the one to write first**; the other four are
corrections, and this is the gap.

## D5 — Two screenshots on the newcomer's path

The owner asked for two, with his privacy respected and no local secrets
exposed. Both already exist, staged outside git at
`.git-exclude/staging/getting-started-screenshots-2026-10-14/`: the first-run
wizard's welcome step and the admin sign-in page — the newcomer's actual first
two screens.

They were rendered by headless `chromium` with `--user-data-dir` pointed at a
throwaway directory, so no browser profile, history, bookmarks, tabs or
signed-in account was touched or reachable; a headless render carries no browser
chrome, so the setup token in the URL is not in the image; and the data is a
throwaway SQLite file in a scratchpad, so no real account, address or hostname
appears. The architect viewed both before staging them.

**Any future screenshot is produced the same way and checked the same way
before it is committed.** That sentence is the durable part of this decision;
the two images are its first instance.

## Non-goals

- **`sui-id --help` advertising five flags that stop the program starting.**
  Real, and already [RFC 107](../proposed/107-cli-help-completeness.md), whose `:16`
  names the same mechanism the review rediscovered — `find_subcommand` treats
  only `--config`, `--to` and `--from` as value-taking. It is a code change and
  it is that RFC's. This one does not touch `crates/`.
- **Restructuring the documentation.** The nav order stays as it is.
- **Any claim that sui-id is or is not ready for a particular use.** D1 records
  what the project already says about itself in `ROADMAP.md`, in one place
  instead of two that disagree.
