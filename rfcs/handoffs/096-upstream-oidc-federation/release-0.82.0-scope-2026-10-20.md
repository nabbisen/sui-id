# `0.82.0` — scope, proposed for authorization

> ## Whose turn it is: **not the architect's.** This needs authorization.
> It does **not** wait on the cycle E decision, and §A says why.

> ## **AUTHORIZED** 2026-10-20 — both asks in §F.
> The cut is authorized on §B's measured severity. The §D pre-publish audit
> step is approved, with a standing constraint on how it is implemented —
> **`@nabbisen`, 2026-10-20**: *"be careful not to damage development cycles and productivity. I mean it
> will harm the project if some check for the release gate only is processed
> at each RFC, each implementation or each review."* Implemented as one
> reading at the cut — not a `[gates]` lane, not part of Level B, and not
> evidence any round must produce. A3.4 passes.

**Preparation dispatched** 2026-10-20 —
[`release-0.82.0-preparation-2026-10-20.md`](release-0.82.0-preparation-2026-10-20.md).

**Proposed.** 2026-10-20 JST, by the architect.
**Prompted by.** *"Is it time to scope 0.82 now?"* — and the honest first
answer was no. Measuring it turned up something that changed it.

## A — on content alone the answer is no; on the audit lane it is yes

**Nothing has accumulated.** Seven commits since the `0.81.0` tag, exactly
**one** touching `crates/`: the three tidies (`2f4511c`), which are an internal
dedup, a module move, and an error label on a path no caller reaches.
`[Unreleased]` is **empty**, by our own decision that none of it is
operator-visible. Cutting on that would repeat precisely what we declined at
stage 10a: a release with nothing an operator can use.

**But the Security audit lane is red, and has been since at least
2026-10-14 — including on `ef52e4f`, the commit tagged and published as
`0.81.0`.** I obtained Level B on that commit, confirmed 27 jobs and 0
skipped, and tagged. `Security audit` is a **separate workflow**, not in
`[gates]` and not part of Level B, so the documented release process passed
with it failing and I did not look.

**That is a gap in the release process and it is mine.**
`docs/src/contributing/release-process.md`'s pre-publish checklist requires
`check-published-versions.py` — added by RFC 131 D7 for exactly this
"nobody was looking" failure — and does not require the audit lane. The
audit workflow's own header already anticipates release-time use:
*"Manually via the Actions tab if a release is being prepared."* The intent
was recorded; the checklist never carried it. **§D proposes the fix.**

## B — what is actually wrong, measured and deliberately not inflated

`cargo audit` reports **9 vulnerabilities**. They are not equally real for us.

| Crate | Advisories | Reachable here? |
|---|---|---|
| `rustls` 0.23.45 | 4 — one **Medium 4.0**, three Low | **Yes, production** |
| `quinn-proto` 0.11.15 | 5 — two rated **High 7.5** | **No — in the lockfile, not in the build** |

**`rustls` is reachable and it is the one that matters.** It arrives through
`reqwest`, a production dependency of `sui-id-core` — which is federation
egress, this project's own TLS client path. The four:

- **RUSTSEC-2026-0346** (Medium 4.0) — `Debug` impls for client session values
  expose TLS secrets.
- **RUSTSEC-2026-0349** (Low 3.7) — an unauthenticated ServerHello can
  downgrade later connections to classical key exchange.
- **RUSTSEC-2026-0348** (Low 3.7) — brotli certificate decompression can
  allocate up to 1 GiB **before** authentication.
- **RUSTSEC-2026-0347** (Low 2.3) — buffered application data sent to the
  outer server after ECH rejection.

**The two High-rated ones do not apply to us, and I am not going to present
them as if they do.** `cargo tree -i quinn-proto` finds no path into our build
on any target. `reqwest` declares `quinn` as an optional HTTP/3 dependency and
our feature selection does not enable it; it is in `Cargo.lock` because the
lockfile records the union of optional dependencies, and `cargo audit` reads
the lockfile rather than the feature-resolved graph. They are worth clearing
anyway — the fix is free — but a 7.5 that is not compiled into the artifact is
not a 7.5 against this project.

**The two `unmaintained` findings are not failures.** The lane denies `unsound`
and `yanked` but not `unmaintained` — settled 2026-09-05, and recorded in
`.github/workflows/audit.yml`'s own comment with its reasoning. `proc-macro-error2` disappears on update anyway;
`paste` remains and stays an allowed warning.

## C — the fix is a lockfile update, and that is the whole release

`cargo update --dry-run`, measured:

```
Updating  rustls        0.23.45 -> 0.23.46     (clears all four)
Updating  quinn-proto   0.11.15 -> 0.11.19     (clears all five)
Removing  proc-macro-error2 v2.0.1
```

Plus the usual transitive movement — `hyper-rustls`, `rustls-webpki`,
`tokio-rustls`, `rustls-platform-verifier` and others.

**No manifest change.** Cargo resolves to *"highest Rust 1.95 compatible
versions"*, so the MSRV floor holds by construction rather than by hope — but
it is confirmed in the round, not assumed.

**This is squarely inside the feature freeze.** The freeze admits four kinds of
work and *"dependency and security maintenance"* is one of them by name. So
**this does not wait on cycle E**, which is the thing that would otherwise gate
any scoping, and it is the reason the answer to the original question is yes.

## D — the process fix, proposed

**Add the audit lane to the pre-publish checklist**, beside
`check-published-versions.py`, as a required step whose output is evidence:
run `Security audit` on the exact commit being tagged and require it green,
or require each finding to be recorded with a reachability assessment and a
decision — the shape §B uses above.

Without it the release process stays able to pass on a commit whose security
lane is red, which is what happened to `0.81.0`. **It is a proposal, not a
change I have made**: it edits a document RFC 131 D4 holds a gate over, and
the rule it adds is the kind I have recorded as mine-to-propose and not
mine-to-enact.

## E — what `0.82.0` would contain, and what it would not

**Contains:** the lockfile update, its `CHANGELOG` entry under `### Security`
naming the four reachable `rustls` advisories with their reachability stated,
and the three tidies that are already on `main` — which finally gives them a
released version to belong to, without their being the reason for the cut.

**Does not contain:** anything from cycle E, which is undecided; RFC 097 or the
threat model, still blocked behind 096-B2 behind M2b; stage 14, which needs an
upstream and credentials.

**The `### Security` entry is drafted where §B of the `0.81.0` proposal says
such entries belong** — in `.git-exclude/`, for a severity-and-wording call
before it is committed — **not written straight into `CHANGELOG.md` with the
stage.** That rule change is still unanswered, so I am following the proposed
version rather than the one that caused the problem. If it is rejected, the
entry moves.

## F — what I am asking

1. **Authorize a `0.82.0` dependency and security maintenance cut**, on §B's
   measured severity rather than on nine raw advisory counts.
2. **Rule on §D**, the pre-publish audit step — or fold it into §B's
   outstanding rule change, since both are about the same failure.

Neither blocks the other, and neither blocks cycle E.
