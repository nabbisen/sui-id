# RFC 140 stage 1 — the three link fields

> ## Whose turn it is: **the implementation role's.**
> [RFC 140](../../accepted/140-application-identity-the-operator-vouches-for.md),
> **Accepted 2026-10-27**. §4 stage 1 only — **the logo is stage 2 and
> §5's decision is still open**, so nothing in this round touches
> `logo_uri`. Cycle H item 2.

**Dispatched.** 2026-10-28 JST, by the architect.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 1 — first, and before anything else in this round

**`origins_before` must be read inside its own transaction.** Wave 3's
`commands::update_federation_provider_allowed_origins` takes it as a
caller-supplied `usize` (`commands.rs:5881`), so a Class-A record
carries an attribute the transaction never verifies. `origins_after` is
computed from the string being written and is exact; `origins_before`
is not.

It is unreachable today — the one caller is single-threaded startup —
and that is exactly why to fix it now. RFC 096-B2's administrator path
is concurrent, and the parameter invites the stale value.

**Have `update_allowed_origins_within_tx` read the current
`allowed_origins` in the same transaction, before its `UPDATE`, and
return the before-count. Then remove the `origins_before` parameter
from the command.** It is new and unreleased, so removing it now costs
nothing; after `0.86.0` it is a breaking change to a published crate.

**This lands before the cut even if the rest of the round slips.**

## 2 — a conflict RFC 140 §1 did not know about, and it is mine

RFC 140 §1 says validation *"reuses
`http::dynamic_registration_validation::validate_application_uri`"* —
HTTPS only, well-formed, length-bounded — and calls it *"one function,
one rule, both callers."*

**That is not the whole picture, and I wrote it without reading the
store.** `clients::update_app_identity` already enforces its own rule,
`is_valid_app_uri` (`repos/clients.rs:546-555`): HTTPS, **or**
`http://localhost`, **or** `http://127.0.0.1`. Two rules exist today
for the same four fields.

**Resolve it this way unless you can show it wrong:**

- **Validate administrator input at the HTTP layer with
  `validate_application_uri`** — HTTPS only. These URLs are rendered to
  **end users** on a consent screen; a `localhost` link there is
  useless at best.
- **Leave `is_valid_app_uri` exactly as it is**, as the store's
  backstop. It is laxer than every caller that reaches it, which is the
  right direction for a defence in depth.
- **Do not "harmonise" the two by loosening the HTTP rule.** That is
  the one change here that would make things worse, and it is the
  tempting one because it removes an apparent inconsistency.

**Measure whether `is_valid_app_uri`'s localhost allowance is reachable
from any current caller** and say so in your return. My reading is that
it is not — dynamic registration validates HTTPS-only above it — but I
have been wrong about this file once already this round.

## 3 — what to build

### 3a — three fields on the admin client-edit form

`EditClientForm` (`http/handlers/admin/clients.rs:365-376`) gains
`homepage_uri`, `privacy_policy_uri` and `tos_uri`, each
`#[serde(default)]` like its neighbours, and the edit page
(`sui-id-web/src/pages/clients.rs`) gains the three inputs. **Empty
means unset**, not empty-string — these columns are nullable and
"unrecorded" is a real state.

**`logo_uri` is not among them.** Stage 2, §5 undecided.

### 3b — the write goes through C10

A sealed Class-A command over
`clients::update_app_identity_within_tx`, emitting
`client.app_identity_changed` — the event the inventory has declared
and nothing has ever emitted. Pattern: `commands::update_client_form`
(C03).

**Attributes: which of the three fields changed**, as a set or as
booleans. **Not the URLs themselves** — same rule as C03's counts and
C24's, and for the same reason: an unbounded caller-supplied string has
no place in a hash-chained record. A reader needs to know *that* the
terms-of-service link changed and *by whom*; the value is in the row.

**`update_app_identity` takes all four URIs**, including `logo_uri`.
The command must pass the **existing stored** `logo_uri` through
unchanged — do not let a stage-1 edit blank a column stage 1 does not
manage.

### 3c — the consent screen renders them, by provenance

`render_consent` already gates `logo_uri` and `homepage_uri` on
`registered_via`. **`privacy_policy_uri` and `tos_uri` are not rendered
at all today** — add them under the same gate, beside the scope list
where a user deciding whether to grant access can reach them.

Three i18n labels, all locales.

### 3d — the inventory and the matrix

C10 `sealed = false → true`, `event`/`descriptor` by `--fix`. A matrix
row and a `docs/src/reference/audit-events.md` row for
`client.app_identity_changed`. G13 reads every backtick-quoted dotted
name in the whole document.

## 4 — tests

Per the four this programme now expects, each with its mutation and the
assertion failure: **rollback**, **exactly-once**, **`NotFound`**, and
**one e2e assertion through the real admin edit route** asserting the
audit record and not only the values.

**Plus two this stage owes specifically:**

- **A dynamically registered client's policy and terms links never
  render**, and an administrator-registered client's do. Model on the
  existing `homepage_uri` pair. Include the counter-test that would
  also pass if the links were dropped for *everyone*.
- **A stage-1 edit does not blank `logo_uri`** (§3b). Seed a client
  with a `logo_uri`, submit the edit form, assert it survives.

## 5 — what not to do

- **Nothing touching `logo_uri`'s own editing** — stage 2, undecided.
- **Do not loosen `validate_application_uri`** (§2).
- **No URL in any note field** (§3b).
- **Write the `CHANGELOG` entry** — cycle H's standing instruction, and
  this reaches an operator: administrators can now set three
  application-identity links, and they appear on the consent screen for
  administrator-registered clients only.
- **No release work.** No tag, no publish, no `workflow_dispatch`, no
  Level B claim.

## 6 — what to return

- §1's change, first, with its own mutation evidence.
- Your §2 measurement — whether `is_valid_app_uri`'s localhost branch is
  reachable from any caller — and the rule you applied.
- **`bash scripts/local-matrix.sh`'s output** and the count it printed.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.
- Per test, the mutation and the assertion failure it produced.
- Any public signature change, named in prose — **including the
  `origins_before` removal**, which is one.
- G20's reported count. It should move **46 → 47**; report what you get.

Return it as a review request in `.git-exclude/review-requests/`. Leave
the working tree uncommitted.
