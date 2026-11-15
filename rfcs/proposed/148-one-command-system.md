# RFC 148 — One command system a newcomer can find their way through

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** This RFC Accepted, with its §6 questions
answered. Under the operator-experience track, nothing is built until the
track's schedule and every one of its RFCs exist (ROADMAP, *Active plan*,
2026-11-15).
**Closure prerequisites.** All of the following:
- Every command the binary dispatches is declared once, in one registry,
  and its dispatch, its help and its reference documentation are generated
  from that registry. A test fails if any of them diverges.
- Every pre-148 form still works, and prints a one-line notice naming its
  new form.
- `sui-id` run with no configuration, in a terminal, offers to create one.
  Without a terminal, it prints the guidance and exits non-zero, never
  waiting for input.
- `sui-id init` never overwrites a file.
- Each of these is tested through the binary, and mutated to show the test
  can fail.
**Tracks.** The operator-experience track, Track C (structure): the command
line.
**Touches.** `crates/sui-id/src/main.rs`, `crates/sui-id/src/cli.rs`,
`crates/sui-id/src/runtime/config.rs`, `docs/src/reference/` (a generated
CLI reference), `docs/src/getting-started/quick-start.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/148-one-command-system/README.md`](../handoffs/148-one-command-system/README.md)

## Summary

Raised by the owner on 2026-11-15, after the first run:

> The command system, subcommand hierarchy, and help message collection all
> need to be organized. For example, the command `sui-id
> --print-sample-config` does not belong to any subcommands, making it
> difficult to find and even more difficult to understand its purpose.

Measured on the `0.91.0` binary:
- **Modes are flags mixed in among the commands.** `--dev`,
  `--print-sample-config` and `--version` sit beside six subcommands and
  nine `admin` actions.
- **`--help` is 190 lines in one flat list**, not grouped by what an
  operator is trying to do.
- **`sui-id help` is an error.**
- **The documentation pointer names repository files**
  (`README.md and docs/src/guides/operators.md`), which a user who
  downloaded the binary does not have.
- **The parser is hand-written** (`main.rs:100-117`, with a list of
  value-taking flags). RFC 107 found it mistaking a flag's value for a
  subcommand. It is also where `setup` went missing from the usage list:
  the dispatch and the help are kept separately, by hand.

## Decision

### 1 — the command tree

Grouped by what the operator is doing. Every leaf is a verb or a noun the
operator would say.

```text
Getting started
  sui-id init [--production] [--output PATH]   write a sui-id.toml to start from
  sui-id setup --admin-username NAME           create the first administrator
  sui-id serve                                  run the server

Running it
  sui-id backup create --to PATH [--encrypt]
  sui-id backup restore --from PATH [--force] [--decrypt]
  sui-id backup verify --from PATH [--decrypt]
  sui-id upgrade preview
  sui-id config sample [--production]           print a configuration to start from
  sui-id config check                           validate a configuration, change nothing

Administering
  sui-id admin user unlock --username NAME
  sui-id admin user reset-mfa --username NAME --reason TEXT
  sui-id admin user recovery-link --username NAME --reason TEXT
  sui-id admin master-key rotate [--generate-new-key | --new-key PATH] [--yes]
  sui-id admin metrics-token rotate
  sui-id admin registration-token issue | revoke --id ID | list
  sui-id admin registration-token legacy-candidates

Developing
  sui-id dev [--bind ADDR] [--db PATH] [--seed PATH] …

Help
  sui-id help [COMMAND …]
  sui-id version
```

`--config PATH` stays one global option, valid on every command that reads
the configuration.

### 2 — nothing that works today stops working

Every pre-148 form is kept as an alias:
- `sui-id` with no command (and a configuration present) still serves.
  The documented service unit runs exactly that:
  `ExecStart=/usr/local/bin/sui-id --config /etc/sui-id/sui-id.toml`
  (`docs/src/guides/deployment.md:240`, `operators.md:1186`);
- `--print-sample-config`, `--dev`, `--version` and `--help`;
- `backup`, `restore`, `verify-backup` and `preview-upgrade`;
- every `admin` action.

An alias prints one line to stderr, never stdout, so piped output such as
`> sui-id.toml` is unchanged:

```text
note: `sui-id --print-sample-config` is now `sui-id config sample`; the old form still works.
```

When aliases may be removed is §6's question 2.

### 3 — one registry, three outputs

Each command is declared once: its words, options, a one-line purpose,
longer help, and whether it reads the configuration. Generated from that
one declaration:
- the dispatch;
- `sui-id help` and `sui-id help COMMAND`;
- the CLI reference page in `docs/src/reference/`. A test compares the page
  with a fresh render, so the documentation cannot drift.

This is the pattern used to retire the Kani harnesses in `0.91.0`: one list
generates both the thing and its check. It removes the class RFC 107
found, where a command exists but the help omits it.

### 4 — the first run is a conversation, but only in a terminal

**With no configuration, in a terminal:**

```text
$ sui-id
sui-id 0.91.0 — no configuration found at ./sui-id.toml.

  sui-id needs a configuration file to start. Create one now?
    [l] a local trial  — http://127.0.0.1:8801, data in this directory
    [p] a production template — HTTPS issuer, /var/lib/sui-id; edit before use
    [n] not now

  choice [l/p/n]: l
  wrote ./sui-id.toml (local trial).

  Next:  sui-id serve      then open the setup link it prints.
  Help:  sui-id help       Reference: https://<docs>/reference/configuration.html
```

**Without a terminal** (a service unit, a script, CI): never prompt, never
wait. Print the same facts and the commands, then exit with status 1:

```text
error: no configuration found at ./sui-id.toml.
  Create one:  sui-id init            (a local trial)
               sui-id init --production  (a production template)
  Reference:   https://<docs>/reference/configuration.html
```

**`sui-id init`** does the same without the question:
- it **never overwrites**. An existing file is refused, with its path, so
  `init` cannot destroy a configuration;
- it writes the local-trial profile by default, or the production template
  with `--production`;
- **both profiles are commented**, and are generated from the same
  definitions as `config sample`. They cannot disagree: RFC 147 §4 is this
  RFC's floor.

`setup` and every command that needs the configuration print the same
guidance, rather than a bare *"No such file"*.

### 5 — help that is short first and complete on request

- **`sui-id help`** (and `--help`) fits one screen: the tree of §1, one line
  per command, then `sui-id help COMMAND` for detail.
- **`sui-id help COMMAND`** carries what the 190-line page carries today:
  every option, the environment variables that command reads, an example,
  and a link to its reference page.
- **An unknown command** names the nearest known one (*"did you mean
  `sui-id backup verify`?"*), then points to `sui-id help`.
- **Documentation is named by URL**, not by repository path, because the
  binary is installed without the repository.

This applies the owner's own caution about noise (*"if you overdo it, the
screen becomes noisy, so it is necessary to narrow down what is always
displayed"*): the first screen is the map, and the detail is one command
away.

## 6 — questions for the owner

1. **The parser.** It can be written here, table-driven, from the registry
   of §3. Or it can be [`clap`](https://crates.io/crates/clap), the
   widely reviewed Rust parser, which generates subcommands and help itself.
   `clap` is **not** in `Cargo.lock` today, so it adds dependencies, and the
   project's supply-chain gates and MSRV apply to them. The architect
   recommends **`clap`**, as the project already prefers reviewed code over
   its own on security paths (the hand-written parser is where RFC 107's
   bug was). The cost is the new dependencies, which is your call.
2. **The aliases' lifetime.** Kept until `1.0`, with the notice of §2 from
   the first release, is the architect's recommendation. Service units and
   scripts written against today's forms then keep working through the
   whole `0.x` series.
3. **The first-run question** (§4) asks only in a terminal. Whether it
   should also ask when the configuration is missing, but `--config` was
   given explicitly, is open. The architect recommends no: an explicit
   path is a deliberate choice, so the answer is an error that names it.

## What this RFC does not do

- It changes no command's behaviour beyond its name, its help, and the
  first-run guidance.
- It adds no function to the server.
- It does not decide where configuration is looked for, beyond the
  current `./sui-id.toml` and `--config`.

## Alternatives rejected

- **Only reorder the existing help.** Rejected. It leaves the dispatch and
  the help kept separately by hand, which is what lost `setup` from the
  usage list, and modes would remain flags.
- **Remove the old forms at once.** Rejected. The documented service unit
  runs `sui-id --config …` (§2), and breaking a running service to
  tidy a help screen is not a trade worth making.
- **Always prompt when the configuration is missing.** Rejected. A service
  unit has no terminal. A prompt there would hang the service, or be
  answered by nothing.
