# M3 closure — can the dev team produce the evidence locally?

> ## Whose turn it is: **yours, for one fact.**
> **`@nabbisen`, 2026-10-25:** *"Is it possible to test it on the local
> machine by dev team ? I'd close if soon possible."* Short answer: the
> dev team can test the **command** locally and already has; they cannot
> produce the **evidence**, because it is a statement about a deployment,
> not about code. But there is a route that closes M3 today, and it needs
> one fact only you have.

**Measured 2026-10-25** against `47eabdf`. Read-only; nothing here changes
code.

## A — what RFC 095 actually requires

`rfcs/accepted/095-dynamic-client-registration-transaction.md:17`, the
closure field, verbatim on the two outstanding items:

> every `legacy_unstamped_dynamic_candidate` has a durably recorded owner
> disposition — disablement, explicit legacy acceptance, or a separately
> audited metadata correction — and a "zero candidates" claim is not
> accepted, because the historical best-effort audit cannot prove
> completeness; and `@nabbisen` accepts the evidence, on the architect's
> assessment of it.

Everything else in that field — validate-first single-transaction
behaviour, invalid requests consuming no use, exactly one concurrent
limited use winning — landed in `0.83.0` and is Level-B green.

## B — the three questions, separated

They have been travelling together and they have different answers.

### B1 — can the dev team test the command locally? **Yes, and it is done.**

`sui-id admin legacy-dynamic-candidates` is read-only
(`crates/sui-id-store/src/repos/audit.rs:772-786` — *"Writes nothing, and
calls nothing that writes"*). Its behaviour is already covered by
`repos/audit/tests_rfc095_stage6.rs` and `crates/sui-id/src/cli/tests.rs`,
including the empty case: `render_legacy_dynamic_candidate_report` is pure
and synchronous precisely so the "no bare zero-count claim" rule is
assertable against its return value rather than captured stdout
(`cli.rs:1020-1023`). All of that was Level B at `0.83.0`.

**So a local run by the dev team would re-prove what the gates already
prove, and would tell us nothing about any deployment.** I do not
recommend spending a round on it.

### B2 — can the dev team produce the closure evidence? **No.**

The evidence is a disposition of rows in a database that actually ran an
affected build. A developer's local database has no such history; a
fixture database has the history someone wrote into it. A report about a
fixture is evidence about the fixture.

### B3 — whose deployments are in scope? **Yours only, and that is already correct.**

Third-party operators are **already discharged**, by documentation rather
than by disposition: `docs/src/guides/upgrade.md:31-49` tells anyone
upgrading to run the command once, states that an empty result is not an
all-clear, and names the three admissible dispositions. You cannot
adjudicate a stranger's database and the RFC does not ask you to.

## C — the measured exposure window, which is what makes this closable

The ambiguity window is `48946f6` (2026-07-02) through `930513b`
(2026-10-02) — the constants the report prints, `cli.rs:1001-1004`.
Translated into versions, which the report does not do and which is the
useful form here:

- **First released version containing the start:** `0.76.3`.
- **First released version containing the fix:** `0.80.0`.
- **Exposed range, therefore: `0.76.3` through `0.79.0`** — published
  `0.76.3`–`0.76.9`, `0.77.0`, `0.79.0`. (`0.76.10`–`0.76.12` and `0.78.0`
  were tagged and never published; they are in the range but nobody could
  install them.)

A deployment that never ran a build in that range **cannot** hold a
candidate, whatever the audit log does or does not contain.

## D — the route, and why it is not a "zero candidates" claim

The prohibition in the closure field is specific and it is worth reading
closely: a zero-candidates claim is refused *"because the historical
best-effort audit cannot prove completeness"*. It bars inferring **absence
of candidates** from **absence of evidence**. It does not bar establishing
that there was never an exposed deployment — that is a claim about
deployment history, not about the audit log, and the audit log's
incompleteness does not bear on it.

So there are two routes, and only you can say which applies:

**D1 — no instance of yours ever ran `0.76.3`–`0.79.0`.** Then the
disposition set is empty *by construction*, not by a silent report. You
record that as the disposition — one sentence naming the deployments
considered and the versions they ran — I assess it, and M3 closes. **This
can be done today.**

**D2 — some instance of yours did run a build in that range.** Then run,
on that machine, against that instance's database:

```
sui-id admin legacy-dynamic-candidates --config <that instance's config>
```

It is read-only and needs no elevated privilege. Every candidate it lists
then gets one of the three dispositions RFC 095 names — disablement,
explicit legacy acceptance, or a separately audited metadata correction —
and the record of that goes in a file I can assess.

**Either way the evidence comes from you.** What I can do once I have it
is the adversarial assessment the closure field makes mine, and then the
RFC-status and ROADMAP work.

## E — what I recommend, and what I did not do

**Recommended: answer D1 vs D2.** If D1, M3 closes today and `0.85.0` can
carry the RFC-status change.

**I did not dispatch a disposal rehearsal.** The dev team could build a
fixture database that does contain candidates and prove the three
disposition paths end to end before you need them. That is real work and
it is worth doing **if D2 holds** — and wasted if D1 holds. Ask me for it
the moment you know it is D2; the dispatch is about half an hour's
drafting and the round is small.

**One thing I corrected while measuring this.** `CHANGELOG.md`'s `0.83.0`
entry said closure needs *"the architect's acceptance of that evidence"*.
That inverts the closure field: the architect assesses, and is not the one
who signs off. Corrected in place, with the correction marked rather than
silently rewritten, since `0.83.0` is published.
