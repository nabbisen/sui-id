# Cycle J stage 6b — build the gate, on the validated design

> ## Whose turn it is: **the implementation role's. Proceed.**
> Your measurement round is accepted. This is a **delta**, not a
> restatement: the design in your return stands except where numbered
> below. Build the bundle you proposed in your §7.

**Dispatched.** 2026-11-04, by the architect.
**Builds on.** `.git-exclude/review-requests/cycle-j-stage6-the-ast-boundary-gate-2026-11-04.md`
(your measurement) and
`.git-exclude/reviewed/cycle-j-stage6-the-ast-boundary-gate-2026-11-04.md`
(my review, which answers both questions you flagged).
**Stage 6's dispatch still governs** everything not changed here —
`rfcs/handoffs/094-transactional-audit/cycle-j-stage6-the-ast-boundary-gate-2026-11-04.md`.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

---

## 1 — the two questions you flagged, answered

**Adding the lane row is not an amendment.** Add it to RFC 094's
*"Gate Matrix lanes owned by RFC 094"* table and to
`[gate_owners]`/`[gate_lane_sources]`. The RFC designed this: *"Adding
the structural-gate lane afterwards is a manifest entry plus a
single-row table in this RFC — the smallest change that can add a
gate"*, and it settles the same question for its predecessor (*"RFC 093
is not amended by this mechanism"*). **My §8's "do not amend" was
under-specified; it meant the stale sentence.** Two constraints from the
RFC's own text: do not rename that heading (*"matched by plain
equality"*), and change the `[gate_lane_sources]` manifest in the same
commit.

**`cargo check` must succeed on the negative fixture, not reject it.**
The compile-negative control is the command type system
(`094-…md:292`), not connections; `Database::with_conn` is `pub`
(`db.rs:66`), so a bare unregistered write compiles — which is why the
AST control exists; and the RFC's next sentence says the two controls do
not overlap. **Evidence is: the fixture compiles cleanly, and
`audit-structure --negative-fixtures` rejects it by name as an
unregistered write.** A fixture that fails to build proves nothing.

## 2 — the `settings.rs` fix: right finding, wrong mechanism

Your fix does not type-check. `migrations::read_stored_version` takes a
raw `&Connection` (`migrations.rs:327`), `with_read` yields `&ReadConn`
(`db.rs:113`), and `ReadConn` has no `Deref` and a `pub(crate)`
constructor (`read_conn.rs:90`). The signature cannot move either —
twenty-odd in-crate callers pass `&Connection` *and* `&Transaction`.

**Build it as a store method instead.** Add
`Database::stored_schema_version()` in `sui-id-store`, implemented with
`with_conn` where raw access is allow-listed, and have the handler call
it. This also deletes the handler's reach into
`sui_id_store::migrations::`, which is the deeper problem: an admin page
asking for "the database's schema version" wants a store query, not a
migrations internal. Keep it inside this change, as you proposed.

## 3 — the detector: drop method-name matching as a primary signal

Your `index.rs` false positive settles it. Build **type-path detection**
(`rusqlite::Connection`/`Transaction`/`Statement`, named, qualified or
import-resolved) and **whole-identifier detection**
(`with_conn`/`with_tx`). Both measured at zero false positives; together
they catch `settings.rs:330` and the mandatory fixture.

**Receiver-type tracking is out of this stage** and is the named,
deliberate gap, along with exhaustive alias/indirection tracing beyond
`backend.rs`'s `ConnFn`/`TxFn`.

**State the gap in two places, as you proposed**: the tool's own output
and `write-authority.toml`'s header. A structural gate that checks less
than its name implies is this RFC's own worst case. PRAGMA, ATTACH /
DETACH and backup/restore are built on the call-site-then-string-check
shape you described; `unsafe_code = "forbid"`
(`Cargo.toml:[workspace.lints.rust]`) already covers raw-handle/FFI, so
record that as *covered elsewhere*, not as *checked here*.

## 4 — the allow-list: write the honest one, and say what it means

**My three-module estimate was wrong; your ~36 files is right.** Write
it exactly. And put your own sentence in the header, because it is the
accurate description of what the gate buys:

> adding a 37th file is the friction point, not touching raw SQL per se.

The header must also carry the RFC's constraint — *"changing it requires
independent review"* — where someone widening the list will read it.
Include `commands.rs`'s inline SQL (`:89-114`, `:344-347`) in the
rationale, since an allow-list justified as "commands.rs only calls
repos" would be false.

## 5 — unchanged from stage 6's dispatch

G17's seven checks `(A)`–`(G)` port and G17 retires in the same change.
`xtask` is a 7th workspace member, `publish = false`, **not** in
`[workspace.dependencies]` — confirm `grep -c '0\.87\.1' Cargo.toml`
still returns 6. `syn = "2"`, MSRV 1.95 confirmed. Negative fixtures
follow A3.2's `scripts/tests/fixtures/` pattern, excluded from the
workspace. `ci.yml` is regenerated, never hand-edited. A3.4 re-verified,
condition 7 included. The dependency condition stays **out** (§F's open
question, `@nabbisen`'s).

## 6 — what to return

A review request at
`.git-exclude/review-requests/cycle-j-stage6b-build-the-gate-2026-11-04.md`:
the baseline; the fixture's evidence from both controls as §1 defines
them; the allow-list and its header; which of G17's seven moved and how
each is tested; the gap statement as it appears in output and header;
the six-version-site check; per-hunk hashes; and the gate summary **run
on the real tree after copy-back**.

**One thing to watch that nothing in the gate set will catch.** You are
replacing the only check over a 103-row contract that this entire cycle
has been measured against. If any of the seven checks is weaker after
the port than before — not absent, *weaker* — no gate will tell us,
because the gate is the thing being replaced. Say explicitly, per check,
that the ported behaviour is equal or stronger, and show one case per
check where it fails.
