# The three tidies

> ## Whose turn it is: **the implementation role's.**
> Nothing waits on `@nabbisen`. These were scheduled as one dispatch, held
> behind 096-B1 and cycle D, and cycle D is released and published.

**Dispatched.** 2026-10-20 JST, by the architect.
**Scheduled by.** [`schedule-2026-10-15.md`](schedule-2026-10-15.md) §6, which
carried them as *"the three tidies… as scheduled"* without naming them. They
were enumerated once, in `.git-exclude/review-requests/schedule-096-a-complete-2026-10-09.md`
item 4, and nowhere since — I had to search the history to recover them, which
is itself a defect in my own documents and is fixed in the same commit as this
dispatch.
**Baseline.** Read the tip with `git log -1`. Nothing is in flight.

## Before anything else: these are small, and two have changed since they were written

Measured 2026-10-20, at `b79d08c`. **The original descriptions are eleven days
old and two of them are now wrong in ways that matter.** Do not work from the
2026-10-09 text; work from this.

No security path is touched. No behaviour an operator can observe changes,
with the single exception noted in tidy 2, which is a *diagnostic* label.

## 1 — one digest, two production implementations

`sha256_hex` exists twice in shipped code:

| Where | Form |
|---|---|
| `crates/sui-id-core/src/oidc/tokens.rs:209` | `pub fn`, documented, `write!("{b:02x}")` into a pre-sized `String` |
| `crates/sui-id/src/http/handlers/dynamic_register.rs:327` | private, `.map(\|b\| format!("{b:02x}")).collect()` |

Semantically identical — both lowercase hex SHA-256. The core one is already
`pub`, so the handler can use it and its own copy deletes.

**Correction to the original text, which said "the tree holds two
implementations".** It holds **eight**: the two above plus six test-local
copies —
`crates/sui-id-store/src/commands/tests/runner/dynamic_registration.rs:9` and
five under `crates/sui-id/tests/e2e/`. **Only the two production ones are in
scope.** Leave the test copies alone unless you judge otherwise and say why;
a test defining its own expected-value helper is often deliberate, because
importing the implementation under test would make the assertion circular.
That reasoning is exactly why I am not ordering their removal.

## 2 — `is_64_hex` accepts uppercase, and the sole caller cannot produce it

`crates/sui-id/src/http/nonce_claim.rs:89`:

```rust
fn is_64_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
```

`is_ascii_hexdigit()` accepts `A-F`. `sha256_hex` emits only `a-f`, and
`validate_nonce` compares the two with `ct_eq` on raw bytes, which is
case-sensitive. So a 64-character **uppercase** expected digest passes the
shape check and then fails the comparison, returning `NonceError::Mismatch`.

**Why that is worth fixing even though nothing can reach it.** The twenty-line
doc comment above `validate_nonce` justifies the shape precondition on exactly
one ground: that a caller bug should *"fail loudly with its own named error
rather than silently taking the length short-circuit's fast 'not equal'
path"*. An uppercase digest is precisely a caller bug, and it is the one shape
the precondition silently misclassifies — reported as `Mismatch`, which reads
as *the token lied*, when the truth is *our own input was malformed*. In an
incident that sends someone hunting an attacker instead of their own code.

**It cannot fire today, and the dispatch says so plainly rather than
overselling it.** I traced the only production caller:
`federation.rs:1153` passes `hex_lower(&claimed.nonce_sha256)`, whose own doc
comment commits to lowercase, and `nonce_sha256` is a 32-byte `BLOB`, not hex
text. So this is a trap laid for the *second* caller, not a live defect.

Fix the predicate to accept lowercase only. **Add a test that an uppercase
64-character digest returns `ExpectedDigestMalformed`, not `Mismatch`** —
without it the change is invisible and the next person reinstates
`is_ascii_hexdigit` as a "harmless widening".

## 3 — two generic JSON helpers live in `jwks`, and `discovery` reaches across for them

`crates/sui-id/src/http/jwks.rs:172` and `:192` define `nesting_depth` and
`first_repeated_member`, both `pub(crate)`. `discovery.rs:438` and `:446` call
them as `crate::jwks::nesting_depth` / `crate::jwks::first_repeated_member`.

Neither is about JWKS. `jwks.rs:166`'s own comment already concedes it:
*"both this and `first_repeated_member` are generic JSON-structure"* helpers.

Move them somewhere that says what they are — a small shared module is the
obvious shape, but **you choose the destination and name, and say why.** Keep
their tests with them. `jwks` and `discovery` both import from the new home;
neither keeps a copy.

## 4 — what to return

The usual package, plus:

- **Tidy 1**: confirmation that only the two production copies moved, and
  your reasoning if you concluded any test-local copy should also go.
- **Tidy 2**: the new test, and the mutation showing it fails when the
  predicate is widened back.
- **Tidy 3**: the destination you chose and why.
- The full matrix. This touches `crates/`, so Level B scope.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the **last**
  commit of the round, baselined on the tip at that moment.

**A `CHANGELOG` entry only if you conclude something here is user-visible.**
My reading is that none of it is — tidy 2's change alters an error *label* on
a path no caller can reach. If you disagree, write the entry and say why;
that is a judgement about what an operator sees, which is yours to make from
the code.

Leave the working tree uncommitted. One open dispatch at a time; this is it.

## 5 — what this does not do

It closes no RFC and no milestone. It is maintenance, scheduled as the last
item of the 096-B1 sequence, and it was held precisely so that none of it
would widen a security-path stage.
