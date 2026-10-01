# RFC 126 — independent design review

**RFC.** [RFC 126 — Password hashing must not block the request runtime](../../done/126-password-hashing-must-not-block-the-runtime.md). **Accepted 2026-09-30, design unreviewed** (its own header states this).
**Handoff reviewed against.** [`rfcs/handoffs/126-password-hashing-must-not-block-the-runtime/design-review-request.md`](../../handoffs/126-password-hashing-must-not-block-the-runtime/design-review-request.md).
**Baseline.** `7262354` or later; reviewed at current `main`.
**Reviewer.** Mid-capability model, implementer role. I found this problem while reviewing RFC 123's design, and RFC 126 exists because of that finding — a closer involvement than usual, disclosed here as the request asked.
**Scope.** Read-only. No tracked file changed. No live instance touched.

---

## 1. Confirmed, and one correction to the RFC's own text

- **Argon2, unchanged since RFC 123's measurement:** `authn/password.rs:11-13`, `m_cost = 64 MiB`, `t_cost = 2`, `p_cost = 1`. `main.rs:22` is still a plain `#[tokio::main]`, no `worker_threads` override.
- **`spawn_blocking` usage, checked precisely — one citation in the RFC's own body is wrong.** `grep -n "spawn_blocking("` (the literal call, not the word in a comment) across `sui-id-core` and `sui-id` finds it in exactly **two places, both in `sui-id-store/src/backend.rs`** (`:146`, `:159`, both wrapping SQLite work). **`authn/hibp.rs` does not call `spawn_blocking` today.** Its own doc comment explains why: *"RFC 070 (v0.57.1): replaced ureq 2 (synchronous, `spawn_blocking`-wrapped) with reqwest 0.12 (async, no wrapper needed)"* (`hibp.rs:88-90`) — that sentence describes a **past** design RFC 070 removed, plus a comment at `:239-241` advising a *hypothetical future* synchronous `HibpClient` implementation to wrap itself, not a description of current code. RFC 126's own body text says *"`sui-id-store/src/backend.rs` uses it for database work, and `authn/hibp.rs` uses it too"* — the second half is not true today. The design-review-request's own item 5 gets this right (cites only `backend.rs`); only the RFC's body and its "Independent design review" field carry the error. **This doesn't weaken the finding** (the load-bearing claim — that `verify_password`/`hash_password` are never wrapped — is still correct) but the RFC should not cite a second precedent that isn't there.
- **Call sites: 24, not "roughly 20"** (close enough that I wouldn't call it wrong, just more precise): every `verify_password`/`hash_password` call outside `password.rs` itself and test modules. Enumerated in full for §2.

## 2. D2 — where the boundary goes

**The API-level boundary is achievable, and the obstacle is not the one the RFC anticipated.** Of the 24 call sites, **23 sit inside functions that are already `async fn`** — converting `verify_password`/`hash_password` to `async fn` (wrapping `tokio::task::spawn_blocking` internally, `password: &str`/`stored_phc: &str` cloned to owned `String`s before the `move` closure) means every one of those 23 needs only an added `.await`, nothing else. I checked each enclosing function directly rather than assuming from the call site's file: `me_security::change_password_self`, `identity::admin::clients::{create_client, rotate_client_secret}`, `setup::create_initial_admin_inner`, `oidc::authorize::authenticate_client`, `oidc::oauth_token::authenticate_client`, `authn::session::login_with_mfa`, `authn::step_up::verify_current_password` (or equivalent), `account::forgot_password::consume_and_reset_password`, `dynamic_register::dynamic_register`, `runtime::dev_mode::apply_seed`, and `authn::mfa::{confirm_enrollment, regenerate_recovery_codes}` are all `async fn` already.

**The one real obstacle is a sync iterator adapter, not a borrowed value.** `authn/mfa.rs:363`, inside `match_recovery_code`:

```rust
let Some(i) = hashes
    .iter()
    .position(|h| verify_password(candidate, h).is_ok())
```

`Iterator::position` takes `FnMut(Self::Item) -> bool` — a synchronous closure. If `verify_password` becomes `async fn`, **this does not compile**, regardless of whether `candidate`/`h` are borrowed or owned — the RFC's own framing ("a caller needs a borrowed value across the boundary, which is what would force the worse option") describes an ownership problem, but the actual obstacle here is a **control-flow shape** problem: a synchronous iterator combinator cannot host an `.await` at all. Ownership is not what's blocking this; the closure's non-async-ness is. This still falls in the bucket D2 asked about ("if that is not achievable... the RFC says so and names what each call site must then do instead"), just for a more specific reason than guessed.

**Fix, concretely:** replace the `.position(...)` with an explicit loop:

```rust
let mut matched = None;
for (i, h) in hashes.iter().enumerate() {
    if verify_password(candidate, h).await.is_ok() {
        matched = Some(i);
        break;
    }
}
let Some(i) = matched else { return Ok(None); };
```

This is a small, mechanical rewrite of one function, not a design change — I'm naming it because "the boundary is in one place" (D2's decision) is only true if this site is also fixed; leaving it as the one un-converted call site would either fail to compile (correctly forcing someone to notice) or, if worked around by keeping a sync wrapper just for this site, would quietly reintroduce the defect D2 exists to prevent.

**Conclusion: D2's preferred design (boundary at the API) is achievable everywhere**, with this one named, small exception fixed in the same patch.

## 3. D4 — the pool's bound, and what happens when it's reached

**Recommend: `spawn_blocking` (D1/item 5's answer below) gated by an explicit `tokio::sync::Semaphore`, not a raw thread-count cap and not a dedicated pool.** Tokio's blocking pool is sized in *threads* (default 512); what actually needs bounding here is *concurrent 64 MiB allocations*, which a semaphore expresses directly regardless of which pool executes the permitted work. Acquire a permit before calling `spawn_blocking`, release it when the hash completes (a guard dropped at the end of the async wrapper function handles this without extra bookkeeping at call sites — the semaphore lives inside `password.rs`, invisible to every one of the 24 callers, consistent with D2's boundary-in-one-place decision).

**A number, derived rather than picked:** tie it to `std::thread::available_parallelism()`, the same source `#[tokio::main]` already uses implicitly for its worker count, with a hard ceiling so a many-core box doesn't get an unreasonably large bound anyway. Something in the shape of `(available_parallelism() * 2).min(16)` is defensible: on the "1-4 core, self-hosted" deployment shape the RFC itself describes, that's 2-8 concurrent hashes, 128-512 MiB peak Argon2 memory — a small, statable fraction of what a small VPS has, and nowhere near Tokio's 512-thread default (which would be 32 GiB). I would not present this as the only correct number — it's a derivation with reasoning, which is what D4 asked for; `@nabbisen` and the implementer can retune the ceiling.

**What happens to a caller when the bound is reached: it queues, and that is the right choice, not a default.** `Semaphore::acquire().await` is itself a proper async wait — it does not occupy a runtime worker thread while pending, so a caller queuing for a permit does not reintroduce the problem this RFC exists to fix. The alternative (rejecting with a 503/429 once the bound is hit) would turn a burst of *legitimate* concurrent sign-ins into visible failures, which is worse for availability than a queued caller waiting slightly longer — and D4's own text says this choice **is** the availability behaviour, not an implementation detail, so the RFC should say "queues" explicitly rather than leave it to whichever primitive an implementer reaches for first. A bounded wait (e.g., `tokio::time::timeout` around the acquire) is a reasonable later refinement against a sustained flood, but isn't required for this RFC's closure prerequisites as written.

## 4. D3 — does the timing equalisation survive a thread hop?

**Yes, and by construction if D2's boundary placement is followed — not as a separate guarantee that has to be remembered.** Every `DUMMY_PHC` call (`session.rs:143,151,164,194`; `oauth_token.rs:283,290,299,305`) and every real verification go through the **identical** `verify_password` function. If the `spawn_blocking` wrap lives inside `verify_password` itself (§2's recommendation), **both a real check and a dummy check pay the identical thread-hop cost automatically**, because there is exactly one code path for "verify a password," real or decoy. This is an argument *for* the API-level boundary that the RFC didn't quite make explicitly: it isn't only "a future caller can't forget it" (D2's stated reason) — it's also "the timing property in D3 doesn't need a second, independently-maintained guarantee that every dummy call site was *also* wrapped." Putting the boundary anywhere else (each call site) would require verifying that fact at each of the 24 sites separately, which is exactly the kind of thing D2 already rejected for the wrapping itself.

**A test that would show this, given the noise a thread hop adds:** run it under a runtime configured with a **known, small worker count** so the phenomenon is deterministic rather than machine-dependent — `#[tokio::test(flavor = "multi_thread", worker_threads = 1)]`. Measure the same way RFC 123's design review did (N samples, p10/median/max, not mean) for a real verification and a `DUMMY_PHC` verification, **after** the `spawn_blocking` wrap exists, and assert the two distributions' medians are close (say, within a few milliseconds — thread-hop scheduling overhead, not zero, but small and *the same* for both branches) rather than asserting exact equality, which would be flaky. The useful assertion is not "no thread-hop cost exists" (it does, and D3 doesn't ask for zero) but "the thread-hop cost lands on both branches, not just one" — which a same-order-of-magnitude median comparison demonstrates without chasing an exact number.

## 5. Is Argon2 the only synchronous work on the request path? — no, in effect, once multiplied

**The only function whose cost is comparable is Argon2, reached through exactly two functions (`verify_password`, `hash_password`) — but one call site pays it up to 8× per request, which the RFC's "~34 ms" framing doesn't surface.** `authn::mfa::match_recovery_code` (`mfa.rs:349-374`) checks a recovery-code guess against every stored hash in a plain loop, `hashes.iter().position(|h| verify_password(candidate, h).is_ok())`, and `RECOVERY_CODE_COUNT = 8` (`mfa.rs:35`). **A wrong guess that doesn't match any of the user's 8 remaining codes costs up to 8 × ~34 ms ≈ 270 ms synchronously today** — the worst single-request blocking cost anywhere in this codebase, not the ~34 ms figure the RFC states throughout. This is already inside RFC 126's stated `Touches` (it's a `verify_password` call site), so it isn't a missed surface, but the RFC's severity language should say so: the exposure this RFC closes ranges from ~34 ms to ~270 ms per request depending on which authentication path is hit, not a flat ~34 ms.

**The same loop shape, for `hash_password`, exists at enrollment and regeneration** (`mfa.rs:118-125`, `:154-161`): 8 sequential `hash_password` calls, ~270 ms total, once per enrollment or recovery-code regeneration — rare, not per-login, and I'd recommend **keeping this loop sequential** even after `hash_password` becomes `async`, rather than parallelising the 8 calls with something like `join_all`: parallelising would momentarily demand 8 concurrent permits from §3's semaphore for one enrollment, competing with unrelated concurrent logins for the same bound, to save a latency cost (270 ms once, at enrollment) nobody has asked to be faster.

**Nothing else comes close.** I looked rather than assumed: WebAuthn signature verification, JWT/JWS signing for access and ID tokens, and the master-key-derived AES-GCM operations for session/recovery-code encryption are all sub-millisecond, standard cryptographic operations with no memory-hardness parameter — none is a second instance of "deliberately expensive by design," which is what makes Argon2 different from the rest of this codebase's crypto.

## 6. Is `spawn_blocking` the right instrument, or a dedicated pool?

**`spawn_blocking`, paired with the semaphore in §3 — not a dedicated pool.** The concern item 5 raises (mixing microsecond SQLite reads and 34-270ms Argon2 hashes in the same pool) is real but narrow once the semaphore exists: `backend.rs`'s two call sites are bounded per-connection already (a `Mutex<Connection>` serialises DB access regardless of pool sharing), so the pool contention that would matter is "does a burst of concurrent Argon2 work delay an unrelated DB-touching request's `spawn_blocking` call." With the concurrency bound in §3 kept well below Tokio's 512-thread default (2-8 to 16, depending on the chosen derivation), there is always ample pool headroom left for DB work even at the Argon2 bound's ceiling — the shared pool "matters" in principle, is not observable in practice at these numbers, and does not justify the added complexity of running and sizing a second, dedicated thread pool for one function family. A dedicated pool would be the right call if the semaphore-bounded shared-pool approach were shown insufficient under load; nothing here shows that, and building it pre-emptively would be more machinery than the measured problem justifies.

## Findings, ranked

**High**

1. **D3 holds automatically only if D2's boundary is followed exactly** (§2, §4) — the RFC should state this connection explicitly (boundary-at-API isn't just "can't be forgotten," it's also what makes D3 true without a second guarantee), and should name the one call site (`mfa.rs:363`) that needs a rewrite, not just a signature change, to reach that boundary.
2. **D4 needs a stated instrument (semaphore) and a stated caller-facing behaviour (queues), not just "a bound."** As written, D4 says a bound is needed and what it must avoid, but not what primitive enforces it or what a caller experiences when it's reached — and D4's own text says that choice **is** the availability behaviour. §3 recommends both explicitly.

**Medium**

3. **The RFC's "~34 ms" framing understates the worst case by up to 8×.** `match_recovery_code`'s loop over all stored recovery codes is a real, already-in-scope call site that can cost ~270 ms synchronously today, not ~34 ms (§5). Worth a sentence in the RFC so a reader doesn't anchor on the smaller number for every path.
4. **One factual correction to the RFC's own body:** `authn/hibp.rs` does not call `spawn_blocking` in the current codebase — only `sui-id-store/src/backend.rs` does (§1). Doesn't change the RFC's conclusion; the citation should be fixed before it misleads a reader checking the claim.

**Low**

5. Recommend the enrollment/regeneration 8-hash loops in `mfa.rs` stay sequential once `hash_password` is async, rather than being parallelised for latency neither RFC 126 nor its predecessors asked to improve (§5).

## Recommendation

**Accept with the changes named above** — none of them change what D1-D5 ask for; they make D2's boundary claim true everywhere (one named rewrite), give D4 an instrument and a caller-facing behaviour instead of leaving both implicit, correct one citation, and sharpen the severity language with a number this review measured against actual code rather than assumed. Implementation is straightforward from here: `password.rs` gains an internal semaphore and becomes `async`, 23 call sites gain `.await`, one (`mfa.rs:363`) gains a small rewrite, and D5's test is a `worker_threads = 1` demonstration that a health-check-shaped request is served promptly during a burst of concurrent, real (non-dummy) authentication attempts — failing before this RFC's change and passing after, per D5's own requirement.

**Entry point of this package:** `.git-exclude/review-requests/rfc-126-design-review-2026-09-30.md`
