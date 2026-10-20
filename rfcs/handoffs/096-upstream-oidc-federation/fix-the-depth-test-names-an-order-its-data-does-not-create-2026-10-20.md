# The three tidies — fix round: the depth test names an order its data does not create

> ## Whose turn it is: **the implementation role's.**
> Nothing waits on `@nabbisen`. One item, one line of test data.

**Dispatched.** 2026-10-20 JST, by the architect.
**Round.** Fix round on the package returned 2026-10-20.
**Baseline.** **Read the tip with `git log -1`.** Your working tree is
untouched and stays uncommitted — `nonce_claim.rs` md5 `e85e53d2…`,
`json_shape.rs` md5 `a64a2f9d…`, as you left them. Build on it.
**Review result.** `.git-exclude/reviewed/the-three-tidies-2026-10-20.md`.

**Tidies 1 and 2 are accepted outright, and tidy 3's code is accepted.** The
destination choice is better than the one the old comment contemplated, the
error-type collapse is behaviour-preserving — I checked the original's error
paths rather than taking the package's word — and the stale "promoted in
place" comment travelled and was rewritten instead of being left behind. Your
own mutation of tidy 2 reproduced exactly when I ran it independently.

One item.

## 1 — `depth_follows_the_deepest_branch_not_the_first` does not test that

`crates/sui-id/src/http/json_shape/tests.rs:31`:

```rust
let v = serde_json::json!({"shallow": 1, "deep": {"deeper": {"deepest": 1}}});
assert_eq!(nesting_depth(&v), 3);
```

I mutated the walker's object arm to the exact defect the test is named for —
`.map(nesting_depth).max()` → `.map(nesting_depth).next()`:

```
test json_shape::tests::depth_follows_the_deepest_branch_not_the_first ... ok
test json_shape::tests::depth_is_one_more_than_the_deepest_child ... ok
```

Both passed. The mutation was caught, but by
`discovery::tests::validate_discovery_bytes_rejects_nesting_deeper_than_16`
and `jwks::tests::nesting_of_seventeen_levels_is_refused` — the caller-side
tests, at exactly the two removes your §tidy-3 said these tests existed to
remove.

**The mechanism.** `serde_json = "1"` with no `preserve_order`, so
`Value::Object` is a `BTreeMap` and iterates **sorted by key**. `"deep"` sorts
before `"shallow"`. The deep branch *is* the first one iterated, so `.next()`
returns it and the assertion holds. The data never creates the arrangement
the name describes.

### What to do

- **Make the test create the arrangement it names.** Keys whose sort order
  puts the shallow branch first is the one-line version.
- **Then decide whether one-line is enough, and say which you chose.** The
  one-line fix is correct today but silently depends on `BTreeMap` ordering;
  enabling `preserve_order` later would flip it back to insertion order and
  could re-vacuate it without anyone touching this file. Covering **both
  arrangements** — shallow-first and deep-first — removes that dependency
  entirely and is still small. I am not mandating it; the trade is a second
  case against a hidden coupling to a cargo feature, and it is yours to weigh.
- **Re-run my mutation and show it failing**, not a different one.
- **Check the other nine the same way.** I mutated only the object arm of
  `nesting_depth`. `an_array_nests_the_same_way_an_object_does` covers the
  array arm, which has the same `.max()` and the same exposure, and I did not
  test it. Mutate each of the ten and report which fail — if another is
  vacuous I would rather find it now than in the next round.

## 2 — why this earns a round at all, since the code is right

It is smaller than RFC 107's, and I want to be straight about that: there, no
test in the tree caught the defect. Here the caller tests do, so the codebase
has no coverage hole — only these tests do not do the job the package claimed
for them.

It still earns a round because **this is the second time in three rounds that
a test over a structure passed while not testing its own stated subject** —
107's binding test asserted substring containment and passed with `backup`
deleted from the help. Both times the implementation was right and the
evidence was weaker than it read. The transferable rule is the same one the
107 dispatch asked for and this one asks for again: **mutate the specific
thing the test is named after, not a representative of its family.**

## 3 — what to return

The usual package, plus:

- **Which fix you chose** for the ordering dependency, and why.
- **All ten `json_shape` mutations**, each shown failing — or, for any that
  do not, say so and fix the test.
- The full matrix. Level B scope; this touches `crates/`.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the **last**
  commit of the round, baselined on the tip at that moment, not on a hash
  copied from this file.

**No `CHANGELOG` entry.** Unchanged from the first round's reasoning, which
I agreed with: nothing here is operator-visible.

Leave the working tree uncommitted. One open dispatch at a time; this is it.
