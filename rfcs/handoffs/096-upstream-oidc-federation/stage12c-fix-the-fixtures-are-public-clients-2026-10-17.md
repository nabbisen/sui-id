# RFC 096-B1 stage 12c fix round — the fixtures are public clients, and their providers say otherwise

> ## Whose turn it is: **the implementation role's.**
> The decision you declined to make unilaterally is made below. Nothing waits
> on `@nabbisen`.

**Dispatched.** 2026-10-17 JST, by the architect.
**First round.** [`stage12c-the-token-endpoint-auth-method-2026-10-17.md`](stage12c-the-token-endpoint-auth-method-2026-10-17.md);
result in `.git-exclude/reviewed/rfc-096-stage12c-the-token-endpoint-auth-method-2026-10-17.md`.
**Baseline.** `5df0d38`, unchanged. Keep the working tree; your four files
are correct and stay.

**Your code is not what needs fixing.** The post→basic change, the
URL-encode-before-base64 handling, the test that pins it, and the
public-client path are all accepted as they stand.

## 1 — the decision

Every federation fixture is a public client — no `client_secret_enc`
anywhere — while its discovery document advertises only
`["client_secret_basic"]`. RFC 096 `:570` forbids exactly that pairing:

> `token_endpoint_auth_methods_supported` | Contains the configured method;
> **`none` required for public clients**

So the check is right and the fixtures model something the RFC rules out.

**Most federation fixtures become confidential.** Seal a secret into
`client_secret_enc` — `sui_id_store::crypto::seal` is `pub` and takes
`db.key()` — so each fixture matches the `client_secret_basic` its own
discovery already advertises.

**Keep at least one genuinely public**, and add `"none"` to *that* fixture's
advertised methods so it is self-consistent too. Both values `:320` permits
should be exercised; converting everything would trade one blind spot for
another.

**The reason this way round, not the other:** the Basic-header path you just
wrote has **no end-to-end coverage at all** today, because no fixture has
ever sent a client secret to a token endpoint. A conformance fix to the code
that talks to an external IdP, covered only by a unit test, is under-tested
exactly where it matters. Converting the fixtures is what gives it coverage;
adding `"none"` everywhere would leave it with none.

## 2 — what that means in practice

- Pick the representative set. You named twelve test files; you have read
  them and I have not, so **which stay public and which become confidential
  is yours** — the constraint is only that both kinds exist afterwards and
  that each fixture is internally consistent.
- **Do not weaken the check to make tests pass.** If something still refuses
  after the fixtures are consistent, that is another finding.
- Say in the package **which fixture now exercises the Basic header
  end-to-end** — that is the coverage this round is really buying.

## 3 — the baseline I had wrong

`sui-id --lib` is **558**, not the 560 I have repeated across several
dispatches since stage 10b. Yours is the correct figure; I verified it on a
clean tree. Reconcile against 558 and I will carry that forward.

## What to return

1. **The fixture changes**, and which stayed public.
2. **The named test that now sends a client secret to a token endpoint** and
   passes through `basic_auth_header`.
3. **The full matrix, 24 of 24** — this is the round that gets there.
4. **Per-hunk SHA-256** against the tip you name, regenerated after your last
   commit, covering both rounds' files together.
5. **Count reconciliation** against the corrected baseline: `sui-id-core
   --lib` **201**, `sui-id-store --lib` **529**, `sui-id --lib` **558**,
   `sui-id --test e2e` **537**, `sui-id --test compile_fail` **12**,
   `sui-id-i18n` **19**, `scripts/tests` **349**.
6. **Anything wrong with this round.** Last round you found that both my
   dispatch and your own prior package had diagnosed the same failure the
   same wrong way, and said so plainly. That is the most useful thing either
   of us did this week.
