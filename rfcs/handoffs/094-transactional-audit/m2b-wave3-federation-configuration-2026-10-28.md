# M2b wave 3 — federation configuration

> ## Whose turn it is: **the implementation role's.**
> Cycle H item 1 —
> [`cycle-h-and-0.86.0-2026-10-28.md`](cycle-h-and-0.86.0-2026-10-28.md)
> §B. **After this round, the only thing standing between M2b and
> closure is the authority switch.**

**Dispatched.** 2026-10-28 JST, by the architect.
**Governing RFC.** RFC 094, **Accepted 2026-08-27**. M2b wave 3.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 1 — what is there, measured today

Wave 3's inventory rows read as four target-state commands. **Two of
them have a live write; two do not.**

| Row | Live write | Audit today |
|---|---|---|
| **C16** `federation.provider.created` | `crates/sui-id/src/runtime/startup.rs:323` | **none at all** |
| *(unnamed in the inventory)* allowed-origins reconcile | `startup.rs:385` | **none at all** |
| **C17** enable/disable | `federation_provider::set_enabled` — **no production caller** | — |
| **C18** delete | `federation_provider::delete` — **no production caller** | — |
| **C23** policy replace | **no function exists** | — |

Both live writes are **configuration-driven provisioning at startup**,
not administrator actions: a configured provider absent from the
database is created, and a configured allowed-origins list that differs
from the stored one is written over it. Each logs at `info` and records
nothing. `git grep` finds `federation.provider.created` **emitted
nowhere in `crates/`**.

**The allowed-origins write is the one that matters most.** It is an
origin allowlist, it changes on a config edit plus a restart, and the
only trace is a log line that may or may not be retained.

## 2 — the ruling this round is built on

**2026-10-25: RFC 094 owns whether a write and its audit record commit
together; RFC 096 owns what the write does.**

That broke a cycle: the inventory said C16–C18's semantics depend on RFC
096-B2, while RFC 094 `:611-613` makes 096-B2 depend on wave 3. **Seal
today's writes as they stand.** The activation generation, the sealed
preflight and the evidence invalidation are 096-B2's, and 096-B2 then
extends a sealed command rather than converting a bare one.

**Do not implement any of 096-B2's semantics in this round**, however
tempting the inventory's `rationale` text makes it look — those rows
describe a target, and §4 below says what to do with the parts that do
not exist yet.

## 3 — what to build

Two sealed Class-A commands on the pattern wave 4 established — read
`commands::update_client_form` (C03) and `commands::disable_client`
(C05) first.

- **C16 — provider create.** `federation_provider::create_within_tx`
  plus the `federation.provider.created` event, one transaction.
  Attributes: the provider's **slug** and **issuer**. Not the client
  secret, and not `client_secret_enc` — `create` encrypts a secret
  today and none of it belongs in a note.
- **The allowed-origins change.** `update_allowed_origins_within_tx`
  plus an event recording that the allowlist changed. **The inventory
  has no row for this write** — that is a finding in itself, since it is
  a live Class-A-shaped write with no id. **Add a row**, numbered in the
  `C` series after the existing highest, and say in your return what
  number you used and why. Attributes: the **count** of origins before
  and after, never the lists — same rule as C03's, and for the same
  reason.

**Both callers are in `runtime/startup.rs`.** The actor is not a user:
use `for_system_actor(None)` with `system_principal: permitted`, as S05
and C22 do. Startup provisioning has no session.

**Error handling must not change.** Both call sites today log at `error`
and continue — a provider that fails to register does not stop the
server. Preserve that: a failed sealed command must still be logged and
still not abort startup. **Say in your return that you checked this**,
because the natural shape of `?` would change it.

## 4 — the three that have no live write

Dispose of them as wave 4 disposed of C10, C22 and K02 — a decision per
row, not a conversion:

- **C17 and C18** (`set_enabled`, `delete`): no production caller,
  tests only. **Do not remove them** — unlike `signing_keys::retire`,
  these are the write surface RFC 096-B2 is going to build on, and
  removing them would be churn. Leave the functions and leave the rows
  `target-legacy`. **Do not set `sealed_by`** — that field names the
  command that seals a row, and nothing seals these yet. Record in each
  row's `rationale` that the conversion waits for 096-B2, since there
  is no live write to seal and the 2026-10-25 ruling only displaces
  what is actually running today.
- **C23**: no function at all. Unchanged, `target-absent`.

**If you think C17/C18 should be sealed now anyway** — they are
callerless, but sealing them would mean 096-B2 extends rather than
converts — say so with your reasoning. I argued the other way because a
sealed command with no caller is what wave 4 removed twice, but the
case here differs and I would rather hear the argument than have it
assumed away.

## 5 — the inventory, the matrix, the registry

- `contracts/write-commands.toml`: C16 and the new row `sealed = true`;
  `event`/`descriptor` by `--fix`. C17/C18/C23 keep their status, with
  the rationale updated per §4.
- `crates/sui-id-store/src/registry.rs`: `AuditEventKind` has **no
  federation-provider variants** today — add what the two events need.
- `contracts/audit-coverage-matrix.md` and
  `docs/src/reference/audit-events.md`: rows for both events. Remember
  G13 reads **every backtick-quoted dotted name in the whole document**,
  prose included.

## 6 — tests

Per command, each with its mutation and the exact assertion failure:
**rollback**, **exactly-once**, **`NotFound`** where the primitive has
that semantic, and **one e2e assertion through the layer that actually
runs** — which for both of these is **startup provisioning**, not an
HTTP route. Stage 3's `dev_mode_seed_client_secret_patch_is_audited` is
the precedent for testing a startup-path command at the layer that runs
it.

**Plus one carried item**, from the local-matrix round's review:
`scripts/tests/test_local_matrix.py` should assert that the **real**
`ci.yml` yields **exactly one** INVERTED check. Polarity is detected by
`-eq 0` appearing in a step's body; an ordinary step that happens to
contain it would be silently treated as inverted, and a real failure
would report PASS. One assertion closes that.

## 7 — what not to do

- **No 096-B2 semantics** (§2): no activation generation, no preflight,
  no evidence invalidation.
- **Do not change startup's error handling** (§3).
- **No secret, and no encrypted secret, in any note field.**
- **Do not remove `set_enabled` or `delete`** (§4).
- **Write the `CHANGELOG` entry** — cycle H's standing instruction, and
  this one reaches an operator: two configuration writes that left no
  record now leave one. Plain prose, no severity.
- **No release work.** No tag, no publish, no `workflow_dispatch`, no
  Level B claim.

## 8 — what to return

- The number you gave the new inventory row, and why (§3).
- Your §4 reasoning if you disagree with mine on C17/C18.
- Confirmation that startup still does not abort on a provider failure
  (§3).
- **`bash scripts/local-matrix.sh`'s own output** — the round that built
  it landed in `d6fc6f9`, so this is the first round that should use it
  instead of an enumerated list. Report the count it prints.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.
- Per test, the mutation and the assertion failure it produced.
- Any public signature change, named in prose.
- G20's reported count.

Return it as a review request in `.git-exclude/review-requests/`. Leave
the working tree uncommitted.
