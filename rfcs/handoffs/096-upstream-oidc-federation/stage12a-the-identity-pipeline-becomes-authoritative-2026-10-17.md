# RFC 096-B1 stage 12a — the identity pipeline becomes authoritative

> ## Whose turn it is: **the implementation role's.**
> Nothing in this dispatch waits on anyone. The account-lock question is
> Dispatch B's, not this one's.

**Dispatched.** 2026-10-17 JST, by the architect.
**RFC.** 096 — **Status: Accepted**.
**Supersedes.** [`stage12-remove-the-two-legacy-paths-2026-10-17.md`](stage12-remove-the-two-legacy-paths-2026-10-17.md),
split in two on the implementation role's measured recommendation. Its §5
becomes Dispatch B and goes out after this lands; its §4 is struck (see
below).
**Baseline.** `ee57b76`, unchanged. Hash against the tip you measure at and
name it.
**Release.** Cycle D is gated on the *pair*. This one does not cut it.

## 0 — the split, and why it was yours to propose

You argued it from `complete_federated_signin` taking a plain `user_id`, so
neither direction depends on the other. That is stage 10's shape rather than
stage 11's, and it is why the answer is yes. Dispatch B waits until this
lands; one at a time, as always.

## 1 — the trust-on-TLS decode, and the pipeline that already runs

Your §1 found the thing that matters: the verified pipeline is **already
live** on every request — `verify_id_token` → `validate_nonce` →
`validate_identity_claims` → `validate_iat` → `validate_optional_claims` →
`construct_identity_capability` → `resolve_verified_identity` — and its
result is bound to `_resolution` and thrown away, while the legacy block
re-decodes the same token without verifying its signature and decides
everything.

So this is not "remove a function". It is **make the value already computed
the one that acts**, and then the legacy block has no callers left. Your
five-state mapping is the plan; build against it.

The integration gap you named is the work: `derive_username` takes
`&IdTokenClaims`, and the `UserRow` construction reads `id_claims.name`/
`.email`/`.sub` directly. `IdentityCapability` exposes the same values
through `preferred_username()`/`verified_email()`/`name()`/`sub()`. Move them
over. **Say in the package what each accessor returned where the struct field
was read**, because that is the one place a silent semantic change could hide
— an accessor that normalises, trims or defaults differently from the raw
claim would change who gets provisioned under what username.

`decode_id_token_claims` has one real caller (`:1093`). Confirm that again at
the tip before deleting, and say whether the function itself goes or stays.

## 2 — the cookie-replay path

Your field-by-field table is the design. Two things in it to carry through
carefully:

- **The `state` collapse.** Hashing the query's own `state` and looking the
  row up by that hash, instead of unsealing a cookie to `ct_eq` against it,
  is the right simplification — a wrong `state` finds no row, which is the
  same refusal. **It earns a test that a wrong `state` finds no row**, so the
  protection is pinned where it now lives rather than only where it used to.
- **`nonce` and `next` you found already dead.** Removing dead code is in
  scope; say which you removed and what proved it dead.

## 3 — the two discovery items

Folded in here, as you proposed. These are the two 096-A could not reach
because it was forbidden to touch `fetch_discovery`, and this stage is not:
the configured-value cross-checks, and `validate_discovery_bytes`'s tightened
bounds. You said both still need a close read of `discovery.rs` before you
would commit to a design — do that read first and put the design in the
package; if either turns out to be already satisfied, that is a finding, the
same way §4 below was.

## 4 — struck

`:1067`'s "slug-rather-than-internal-id audit note" does not exist. `:1067`
is egress policy; the real rule is **`:967`** — *"Provider slug may be a
configured bounded field in operator logs but not a metric label"* — which is
about logs and metrics, not audit.

You verified the audit half (every call site passes internal ids, matching
`:715`). I verified the half you did not: `metrics.rs` contains no `slug` and
no `provider`, and the two federation metric call sites pass only a
`signin_result` constant. Nothing to fix on either reading.

**The citation was mine**, from the stage plan of 2026-10-09, repeated into
the schedule and the superseded dispatch without being checked. Corrected in
all four places in the same change as this dispatch. Build nothing for it.

## 5 — the CHANGELOG entry

In this package, as the standing rule now has it. One entry for what this
stage changes: the federated callback's identity decision now comes from the
verified pipeline rather than an unverified re-decode, and the sealed-state
cookie is gone. **It is a security entry** — say what was true before, what
is true now, and whether a released version was affected, the way the last
two rounds' entries do.

## What this stage must not do

**No `/admin/login/mfa` wiring.** That is Dispatch B, and the account-lock
question is attached to it, not here.

**No release.** Cycle D needs both halves.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **§1's swap**, with the accessor-versus-field comparison asked for above.
2. **§2's removals**, with the wrong-`state` test, and what proved `nonce`
   and `next` dead.
3. **§3's design**, from the close read, or the finding that one is already
   satisfied.
4. **The CHANGELOG entry.**
5. **Per-hunk SHA-256** against the tip you name, regenerated after your
   *last* commit.
6. **The full matrix, all 24 green.**
7. **Count reconciliation** from `a6524e0`: `sui-id-core --lib` **201**,
   `sui-id-store --lib` **529**, `sui-id --lib` **560**, `sui-id --test e2e`
   **533**, `sui-id --test compile_fail` **12**, `sui-id-i18n` **19**,
   `scripts/tests` **349**.
8. **Anything wrong with this dispatch.** This round you corrected a citation
   I had propagated through four documents and a claim I made about what runs
   in production. Both were mine and both were load-bearing.
