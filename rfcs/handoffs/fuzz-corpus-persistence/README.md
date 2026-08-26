# Make fuzzing accumulate — seed corpus and CI persistence

**Tracks.** RFC 084's stated corpus design, which was never implemented.
**Owner.** Implementation.
**Baseline.** `d5e5402`.
**Why now.** The first full hosted run (`32950029257`, 2026-08-26) fuzzed all six
targets at 100,000 iterations each and found nothing. That result is worth less
than it looks, for a reason this change fixes.

## The problem

**Every run starts from an empty corpus.** `fuzz/corpus/` is gitignored, and the
workflow neither restores nor saves it — the only corpus step uploads artifacts
*on failure*. So each dispatch begins from zero and nothing compounds.

The cost is visible in the run's own numbers: `accept_language` built a
**313-entry corpus from scratch** inside its 100,000 runs. Most of the budget went
on rediscovering the shape of valid input rather than probing states behind it.
Six targets, 600,000 iterations, and the next dispatch will relearn all of it.

## A contradiction to resolve first

RFC 084 §Design says:

> "Corpora seeded from real request shapes; corpus committed under
> `fuzz/corpus/` (small, curated)."

`fuzz/.gitignore` line 2 says:

```
corpus
```

**Both cannot be true, and the `.gitignore` wins.** This is the third instance of
the same fault in this harness: the `[workspace]` table removed while
`fuzz/README.md` still described a separate workspace; the `fuzz-build` PR gate
described in a comment but never wired to a trigger; and now a committed corpus
specified in the RFC and ignored by git.

**RFC 084's plan was not merely unimplemented — it does not work as written.**
`fuzz/corpus/<target>/` is cargo-fuzz's *working* directory: it writes generated
inputs there on every run. A directory cannot be both "small, curated" and the
place a fuzzer dumps thousands of machine-generated files. Whoever added the
`.gitignore` line was resolving a real conflict, not being careless.

## The design — separate the two, because they have different lifecycles

| Path | Contents | Tracked |
|---|---|---|
| `fuzz/seeds/<target>/` | **curated** starting inputs, small, human-reviewed | **yes** |
| `fuzz/corpus/<target>/` | cargo-fuzz's generated working corpus | no, unchanged |

Curated seeds are reviewable and deterministic: the same seeds mean the same
starting point on every machine. Generated corpus is disposable and grows without
bound. Keeping them apart is what makes "small, curated" enforceable rather than
aspirational.

**I am amending RFC 084 to record this**, since it changes what that RFC
specified. You do not need to touch the RFC.

## What to implement

**1. `fuzz/seeds/<target>/` for all six targets, tracked.**

Seeds should be *real request shapes*, per RFC 084's intent — a valid
`Accept-Language` header, a well-formed JWT, a realistic authorize query string,
a valid PKCE triple, and so on. A handful each; this is a starting point, not a
corpus.

The current local `fuzz/corpus/*` directories (14–48 files each) are a **source**
but not an answer: they are machine-generated and minimized, not real shapes.
Picking a few that represent distinct valid structures is fine; committing all 146
is not.

**2. Copy seeds in before running**, in the workflow's run step:

```yaml
mkdir -p corpus/${{ matrix.target }}
cp -n seeds/${{ matrix.target }}/* corpus/${{ matrix.target }}/ 2>/dev/null || true
```

`-n` so a restored cache entry is never overwritten by a seed.

**3. Cache the generated corpus across runs**, keyed per target:

```yaml
      - name: Restore fuzz corpus
        uses: actions/cache@<pinned-sha>
        with:
          path: fuzz/corpus/${{ matrix.target }}
          key: fuzz-corpus-${{ matrix.target }}-${{ github.run_id }}
          restore-keys: |
            fuzz-corpus-${{ matrix.target }}-
```

The `github.run_id` key with a prefix `restore-keys` is the accumulate pattern:
every run restores the newest prior corpus and saves a fresh entry, so coverage
compounds instead of resetting.

**The cache action must be SHA-pinned and recorded in `ci/gate-inputs.toml`'s
`[actions]` table.** A3.4 condition 2 requires every workflow SHA to appear
there, and condition 3 requires the converse. `cache_v5` is already declared —
check whether it is the version you want before adding a new row.

## Verify

- `bash scripts/check-gate-inputs.sh --all --policy ci/gate-inputs.toml` — this
  will fail if the cache action's SHA is not in `[actions]`.
- Locally, confirm seeds are picked up: run a target with an empty
  `fuzz/corpus/<target>/`, check the `#0 READ` / `INITED` line reports the seed
  count rather than starting from one unit.
- After the first two hosted dispatches, compare the second run's `INITED`
  coverage against the first's. **If it is not higher, the cache is not
  restoring** — and that is the whole point of the change, so check it rather
  than assuming.

That last check is the acceptance test. A cache that silently misses looks exactly
like a cache that works: the run still passes.

## What this does not do

It does not make fuzzing find the defect we know about.
`decode_id_token_claims` accepts an unverified signature — it returns a wrong
answer, it does not crash, and no amount of corpus depth will surface it. This
change makes the fuzzer better at what fuzzing does, which is not the same as
making it a check on correctness.
