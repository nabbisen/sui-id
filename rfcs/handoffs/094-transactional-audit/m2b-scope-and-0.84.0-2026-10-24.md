# M2b — the wave order, and what `0.84.0` contains

**Written.** 2026-10-24 JST, by the architect.
**Proposal, not a plan of record.** `0.84.0`'s scope is for `@nabbisen` to
settle. **The wave order below is the part worth his attention**, because
it is not the order RFC 094 lists.
**Governing RFC.** [RFC 094](../../accepted/094-transactional-audit-registry.md),
**Accepted**; M2a closed 2026-10-03.
**Measured.** 2026-10-24, against the tip.

## A — the state of M2b, counted

`contracts/write-commands.toml` holds **102 commands; 25 sealed, 77 not.**
M2b is those 77 plus the authority switch — several releases, not one.

RFC 094 `:1093-1096` names four conversion waves: *"server and security
settings; pending sensitive settings; federation configuration; client
administration metadata."*

| Wave | Commands | What is actually wrong today |
|---|---|---|
| 1 — server/security settings | S01–S06 | ~~**Nothing is leaking.** Already atomic~~ — **wrong, corrected 2026-10-24; see §B′.** Four of the six emit **no audit record at all** on their production path |
| 2 — pending sensitive settings | S07–S09 | **4 best-effort appends after a committed write** (`settings/pending_change.rs:121, 184, 211, 242`) |
| 3 — federation configuration | C16–C18, C23 | `target-legacy` ×3, `target-absent` ×1 — unbuilt rather than mis-built |
| 4 — client administration | C01–C10, C21, C22, K02, K03 | **9 best-effort appends after committed writes**, all through one helper |

## B — the finding that reorders them

**RFC 094's listing order puts the safe wave first and the worst one
last.** That ordering is a decomposition in a semicolon list, not a
sequencing mandate, and sequencing is the architect's — but it is worth
saying plainly rather than quietly departing from it.

**Wave 1 is already atomic.** `change_hibp_mode` and its siblings call
`audit::append_within_tx` inside `db.with_tx`. Converting them seals a
typed command over behaviour that is already correct. Real work, no defect
closed.

> ## §B′ — the paragraph above is wrong, corrected 2026-10-24
>
> **I read `change_hibp_mode` and generalised from it without tracing
> which function production calls.** `server_settings` has pairs: an
> atomic `change_*` and a bare `update_*`. The inventory names the
> **bare** one as each row's mutation surface, and the admin settings page
> calls the bare one. The atomic `change_*` pair is reached only from the
> first-time setup wizard.
>
> Measured, per production path:
>
> | | Caller | Audit today |
> |---|---|---|
> | S01 default language | `handlers/settings.rs:93` → `update_default_lang` | **none** — the `.changed` event is emitted only from `setup.rs:392` |
> | S02 HIBP mode | setup only; **not settable from the admin page** | atomic |
> | S03 idle timeout | `handlers/settings.rs:183` | **none** |
> | S04 max concurrent sessions | `handlers/settings.rs:214` | **none** |
> | S05 metrics token | `cli.rs:883` | **none** |
> | S06 SMTP configuration | `handlers/settings.rs` | best-effort, `:507` |
>
> `settings.idle_timeout.changed`, `settings.max_sessions.changed` and
> `settings.metrics_token.rotated` are named in the inventory as those
> rows' rationale and **have no emission site anywhere in the
> workspace** — grepped, zero hits outside the inventory itself.
>
> **So wave 1 is not the safe wave.** Four operator actions — including
> rotating the metrics token, which is a credential, and changing the
> session limits, which are security controls — leave no record at all.
> That is worse than waves 2 and 4, which at least attempt one and can
> lose it.
>
> **The ordering this document recommended is unaffected for `0.84.0`**,
> because waves 2 and 1 are both in it and 2 is already done. What is
> affected is the comparison with wave 4 for `0.85.0`, and the claim that
> stage 3 "closes no defect", which was wrong in both this document and
> the stage plan.

**Wave 2 commits and then logs, best-effort.**
`settings/pending_change.rs:170` consumes the pending row through its own
`db` call — which commits — and the audit append that follows is
`let _ = audit::append(...)`. If that append fails, **a sensitive settings
change has been applied with no record of it**, and the row is consumed,
so there is nothing to re-derive it from. On the binding-failure path the
row is consumed *before* the check, and the denial record is best-effort
too.

**Wave 4 is the same shape, nine times.** `identity/admin.rs:31-46`'s
`audit_ok`/`audit_with_note` is a `let _ = audit::append(...)` after the
fact, and `identity/admin/clients.rs` calls it at `:113, :144, :171, :212,
:262, :285, :321, :392`, with `signing_keys.rs:94` making nine. So a
client created, updated, disabled, soft-deleted, or a signing key retired,
can each commit with the audit row silently dropped. `repos/clients.rs`
has **no** `append_within_tx` at all.

**Risk order is therefore 4 > 2 > 3 > 1** — close to the reverse of the
list.

## C — what `0.84.0` should be, and why not the biggest wave

**Proposal: `0.84.0` = the settings area, complete. Wave 2 first, then
wave 1.** Nine commands, S01–S09, closing the four best-effort defects.

**Why not wave 4 first**, despite it holding the most defects:

- It is roughly twice the size and would be several dispatches before
  anything shipped.
- **M2b's waves do not share M2a's shape.** M2a converted commands whose
  mutation and audit were already adjacent; wave 4's are separated by a
  helper and a layer. The first M2b tranche is where that pattern gets
  established and reviewed, and establishing it on nine commands with a
  helper indirection is a worse place to learn than on three.
- Wave 2 closes the sharper defect even though wave 4 has more of them: a
  consumed pending change is one-shot, so a dropped record is
  unrecoverable, where a client's state can at least be re-observed.

**Why wave 1 comes with it rather than later.** It is six commands that
need sealing and nothing else, it completes the S-series so the settings
area stops being half-converted, and it is cheap precisely because it is
already correct. Leaving it would mean returning to the same files twice.

**S10 (complete first setup)** is an S-row but is bootstrap, not settings
administration. **Named and excluded** so it is a visible decision rather
than an oversight; it belongs with whichever wave takes setup.

## D — what `0.84.0` does not contain

- **Wave 4, client administration.** Proposed for `0.85.0`, with its nine
  sites named in §B now so the sequencing is a choice on the record rather
  than a drift.
- **Wave 3, federation configuration.** Three `target-legacy` rows and one
  `target-absent`; C23 depends on RFC 096 scope that is not settled.
- **The authority switch** (RFC 094 §M2b item 6 — the `syn` AST gate, the
  structural gate becoming blocking, the string gate's downgrade). It must
  come last: it exists to catch a newly introduced bare write, and RFC 094
  `:1101-1105` is explicit that building it before the boundary exists is
  premature.
- **M3's two closure prerequisites**, which remain `@nabbisen`'s and are
  not release content.

## E — the decision this needs

1. **Authorize `0.84.0` as S01–S09**, wave 2 before wave 1.
2. **Or direct otherwise** — in particular, if the nine client-administration
   sites in §B read to you as too sharp to leave until `0.85.0`, say so and
   I will take wave 4 first instead. That is a risk judgement about a
   defect that has been live since those commands were written, and the
   case for either order is above.

Nothing here is dispatched. Whichever is authorized, the stage plan
follows it, and the first dispatch goes out against that plan.
