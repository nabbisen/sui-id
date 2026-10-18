# RFC 107 — one list per level, and a flag's value stops being a subcommand

> ## Whose turn it is: **the implementation role's.**
> Nothing waits on `@nabbisen`. RFC 107 is **Accepted** (2026-10-18), so it is
> implementation-eligible under RFC 000 `:49-50`.

**Dispatched.** 2026-10-18 JST, by the architect.
**RFC.** [107](../../accepted/107-cli-help-completeness.md) — **Status: Accepted**.
**Handoff.** [`README.md`](README.md) — the specification. This dispatch adds
the measurement and the order; where they differ, say so rather than guessing.
**Independent design review.** [`design-review-2026-10-14.md`](design-review-2026-10-14.md)
— by the implementation role, and genuinely independent: the architect
authored RFC 107 and did not review it.
**Baseline.** `1a49e95`. Read the tip with `git log -1` and confirm.

## 1 — measured at `1a49e95`, so you start from fact

Every figure below I measured for this dispatch rather than copying from the
handoff. Re-measure anything you intend to rely on.

**The admin level really does keep three hand-written lists**, and they are
the ones the handoff names:

| # | Where | What it holds |
|---|---|---|
| 1 | `cli.rs:101-109`, the `match` | six arms: `unlock-user`, `reset-mfa`, `issue-recovery-link`, `rotate-key`, `rotate-metrics-token`, `issue-registration-token` |
| 2 | `cli.rs:110-111`, the unknown-subaction message | the same six, hand-written in prose |
| 3 | `print_help`'s usage block (`:625-628`) and description block (`:655-676`) | **four** — `rotate-metrics-token` and `issue-registration-token` are absent |

**The top level keeps two, not three.** `main.rs:72-77` dispatches `backup`,
`restore`, `verify-backup`, `admin`, `setup`; its unknown-subcommand arm
(`:77`) enumerates nothing — it says *"Run `sui-id --help` for usage."* So the
top-level const binds the `match` to the help text, and there is no third list
to fold in. Do not invent one.

**The two missing entries are exactly two.** I checked all six against the
help region by reading it, not by grepping the file as a whole — `unlock-user`
(`:625`, `:655`), `reset-mfa` (`:626`, `:660`), `issue-recovery-link` (`:627`,
`:665`) and `rotate-key` (`:628`, `:674`) are all documented. The RFC's
corrected Summary is right and needs no further correction.

## 2 — `FLAGS_WITH_VALUE` is 3 of 16, and the list must not come from the help text

`main.rs:91`:

```rust
const FLAGS_WITH_VALUE: &[&str] = &["--config", "--to", "--from"];
```

Cross-checking every long flag in `cli.rs` and `main.rs` against its help
placeholder gives **sixteen** that take a value: the three above plus
`--admin-display-name`, `--admin-email`, `--admin-username`,
`--dev-admin-password`, `--dev-bind`, `--dev-client-secret`, `--dev-db`,
`--dev-seed`, `--max-uses`, `--new-key`, `--note`, `--reason`, `--username`.
Nine are bare: `--decrypt`, `--dev`, `--encrypt`, `--force`,
`--generate-new-key`, `--help`, `--print-sample-config`, `--version`, `--yes`.

**Treat that list as a cross-check, not as your source.** I derived it from
the help text, and the help text is the artefact this RFC exists because it
cannot be trusted. **Derive the real list from each flag's own parsing** — the
same rule the handoff already sets for the help entries — and tell me if it
disagrees with my sixteen. A flag wrongly listed as value-taking makes
`find_subcommand` skip the real subcommand, which is a worse bug than the one
being fixed, so this list earns the stricter sourcing.

**One subtlety, so you do not think the scope is padded.** The defect only
bites for a value-taking flag that appears *before* the subcommand, because
`find_subcommand` returns at the first non-flag token. `sui-id admin reset-mfa
--username bob` is unaffected — `admin` is returned before `--username` is
reached. What is actually broken today is `--dev`, which has no subcommand at
all, so `sui-id --dev --dev-seed path.toml` reads `path.toml` as one. A
complete list is still the right fix: listing a flag that can only appear
after the subcommand costs nothing, because the function has already returned
by then, and a list that tries to be clever about position would be a fourth
thing to keep in sync.

## 3 — build

Follow the handoff's `Required` section; it is the specification and I am not
restating it. Two additions and one choice:

**The choice it leaves you.** *"An arm with no entry fails to compile or fails
the test; choose the mechanism and say why."* I have no preference and will
not pretend to — **make the call and give the reasoning.** A compile-time
binding is stronger if it is natural here; a test is better than a
contrivance that makes the dispatcher harder to read. Half 2 of this
project's design philosophy is about the person reading the output, and the
dispatcher's readability serves the person reading the code; weigh both.

**`admin issue-recovery-link` already landed** (`b3ee7de`), so the handoff's
"coordinate with RFC 103" note is overtaken: the const must list all six
arms including that one, and the binding test must cover all six. Nothing is
reserved for RFC 103 to add.

**One stale sentence no gate checks.** `cli.rs:95` reads *"admin subaction is
`unlock-user`, but the surface is shaped to grow."* It was true when there was
one; there are six. The surface grew and the comment did not. Fix it in the
same pass — it is the same defect class as the three lists, in prose.

## 4 — documentation, which is half the point of this RFC

- `docs/src/getting-started/quick-start.md:101-103` and
  `docs/src/guides/operators.md:222-223` both state that the `--dev-*` flags
  are **rejected at startup** with `Error: unknown subcommand`. That is true
  today and false the moment this lands. Both must describe the fixed
  behaviour, not carry a warning about a bug that no longer exists.
- `cli.rs:651`: `verify-backup`'s help says it runs *"without writing
  anything"*. It stages a temporary copy of the snapshot for the integrity
  check. Say so, as `docs/src/guides/deployment.md` already does.

## 5 — what to return

The usual package, plus:

- **Which binding mechanism you chose and why**, in a sentence or two.
- **Your derived value-taking list, and whether it matches my sixteen.** If it
  does not, the difference is the interesting part of this round.
- The two `find_subcommand` tests the handoff requires: one running it over
  each value-taking flag followed by a value and asserting no subcommand is
  found; one starting dev mode with each `--dev-*` flag set and asserting it
  is honoured (`--dev-bind` binds the given address, `--dev-seed` reads the
  given file). **Mutate each away and confirm it fails** — a test over a list
  is exactly the shape that can pass while testing nothing.
- A **`CHANGELOG` entry in this dispatch's own commit**, standing since stage
  12. The dev-flag fix is user-visible and belongs under `Fixed`.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the **last**
  commit of the round, not the first. If I land anything between now and your
  return, baseline against my commit and not `1a49e95` — the 12b fix round hit
  exactly that and caught it.
- The full matrix. This touches `crates/`, so Level B: 27 jobs, 0 skipped, all
  24 `[gate_owners]` entries matched both ways.

Leave the working tree uncommitted, as usual. One open dispatch at a time;
this is the only one.

## 6 — what this closes, and what it does not

RFC 107's three closure prerequisites: every dispatched subcommand appears in
`--help`; the lists cannot drift again without a test failing; a flag's value
is never mistaken for a subcommand. **It does not close the RFC** — closing
one needs its own `Closure reviewed on` and `Closure approved by` record per
RFC 000 and RFC 131 D2, which comes after this lands.

RFC 107 joins cycle D (v0.81.0) and **does not gate it**: the cut's own gate,
stage 12, is already met. If this package is not back when the cut is
proposed, v0.81.0 goes without it.
