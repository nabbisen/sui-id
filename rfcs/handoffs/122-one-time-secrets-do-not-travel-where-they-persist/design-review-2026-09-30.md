# RFC 122 — independent design review

**RFC.** [RFC 122 — A one-time secret does not travel where it persists](../../proposed/122-one-time-secrets-do-not-travel-where-they-persist.md). **Proposed.**
**Handoff reviewed against.** [`rfcs/handoffs/122-one-time-secrets-do-not-travel-where-they-persist/design-review-request.md`](../../handoffs/122-one-time-secrets-do-not-travel-where-they-persist/design-review-request.md), [`README.md`](../../handoffs/122-one-time-secrets-do-not-travel-where-they-persist/README.md).
**Baseline.** `73a782a` (`= 8c137bd` plus doc-only commits; no source file differs). Nothing changed by this review.
**Reviewer.** Mid-capability model, implementer role. I confirmed two of the RFC's own findings earlier, in the RFC 120 triage; I did not author the RFC.
**Scope.** Read-only, as instructed. No file in this repository was changed; no request was sent to any running instance — every finding below came from reading the code that runs, and, where noted, from reading Leptos's own escaping behaviour for the one place severity turned on it (item 3).

---

## 1. Confirming the two measurements

**Both confirmed, exactly as stated, at the current line numbers.**

- `crates/sui-id/src/http/handlers/admin/clients.rs:401-431` (`clients_rotate_secret_post`): percent-encodes the new secret and redirects to `/admin/clients/{id}/edit?rotated_secret={encoded}` (`:427-430`). The comment above it (`:423-425`) says the browser history entry "is replaced by the subsequent navigation" — a redirect does not do that; the intermediate URL, containing the plaintext secret, is a real history entry, a real `Location` header, and a real referrer source.
- `crates/sui-id/src/http/handlers/admin/clients.rs:298-331` (`clients_edit_get`): `ClientEditQuery.rotated_secret` (`:292-296`, typed `Option<SecretString>`) is read directly into `ClientEditData.freshly_rotated_secret` (`:323`) with no check that it originated from a rotation this server performed. Rendered at `crates/sui-id-web/src/pages/clients.rs:288-297`.

## 2. The full enumeration — what the RFC found, and five more surfaces

The RFC's own list (module doc, `README.md`) names four `no-store`-present surfaces and four absent. Re-measured, plus the sweep the request asked for (`grep` across `crates/sui-id/src/http` for `Cache-Control`/`SecretString`/`expose_secret`/every field name on the four documented secret types, then every render call in `sui-id-web/src/pages` for the same). One surface — item **G** below — is not in the RFC at all.

| # | Surface | `file:line` | D1 (never a URL)? | D3 (`no-store`)? |
|---|---|---|---|---|
| A | Client secret at creation | `admin/clients.rs:131` (`clients_create`), response built `:207-217` | **Yes** — rendered directly from the POST response, no redirect | **No** — no header anywhere in the function |
| B | Rotated client secret | `admin/clients.rs:401-431` (issuing) + `:298-331` (displaying) | **No** — the one D1 violation the RFC names | **No** on either the redirect or the edit-page response |
| C | TOTP secret + QR | `me_security/mfa.rs:70-116` (`mfa_enroll_start`) | Yes — rendered directly | **No** |
| D | Recovery codes, at enrollment | `me_security/mfa.rs:124-156` (`mfa_enroll_confirm`) → `render_mfa_tab_with_fresh_codes` (`:238-282`) | Yes — rendered directly | **No** |
| E | Recovery codes, regenerated | `me_security/mfa.rs:201-232` (`mfa_regenerate_recovery`) → same shared renderer | Yes — rendered directly | **No** |
| F | Recovery link (admin-issued) | `admin/users.rs:601-717` (`users_recovery_link`) | Yes — rendered directly (already the model to match) | **Yes**, and `Referrer-Policy: no-referrer` too (`:711`, `:712-715`) |
| **G** | **Dynamic client registration's `client_secret`** (RFC 7591) | `dynamic_register.rs:8-256` (`dynamic_register`), returned at `:256` (`Json(resp)`, `client_secret` field at `:65,244`) | Yes — a JSON POST response, not a redirect | **No** — confirmed by exhaustive `grep` for `Cache-Control`/`CACHE_CONTROL` in this file: no hits |

**G is not named anywhere in the RFC or its handoff.** It is reachable with only a bearer token against `client_registration_token` (`router.rs:29-35`, mounted under `token_routes`/`token_cors` — an origin-allowlist CORS layer, not a session check) — the one surface in this whole set that is **not** behind a full admin session. That is a weaker trust boundary than every other surface the RFC measured, and it is exactly the kind of machine-callable endpoint more likely to sit behind an API gateway that logs or caches responses than an authenticated `/admin` page is — which sharpens, rather than only illustrates, the RFC's own closing argument about a log aggregator arriving later.

Surfaces I checked and found correctly out of scope, so the RFC's boundary is not too narrow elsewhere: the SMTP password (settings.rs) is write-only, never redisplayed; the setup wizard's admin password is operator-supplied, not server-generated; signing keys are asymmetric and the private material is never shown; access and refresh tokens are routine API credentials, not "shown once to a human" secrets, and the token endpoint already carries `no-store` (`oidc.rs:524-525`); the metrics token and registration token are CLI-only (`cli.rs`), printed to stderr, never an HTTP surface at all.

## 3. Item 2 — is D1 achievable everywhere, and what does it cost?

**Yes, for every surface, at zero additional engineering cost — because the codebase already proves it.** Surfaces A, C, D, E already render the secret directly from the POST response that produced it; only B (the rotated secret) uses the redirect-with-query pattern the RFC objects to, and B is the *only* one of the six that needs to change transport at all. Fixing it means making `clients_rotate_secret_post` do exactly what `clients_create` (a sibling operation in the same file) already does: fetch the client row and render the edit page directly, instead of redirecting. This is not a new pattern to design — it is copying one this file already contains.

**The cost to the operator, stated precisely, and it is the same cost every option carries, not one this fix introduces.** A response rendered directly from a POST is not re-derivable by a plain reload: browsers show a "resubmit the form" confirmation on refresh, and confirming it would call `rotate_client_secret` *again* — generating a new secret, not redisplaying the old one, and invalidating whatever the operator was trying to copy. This is already true today for client creation (A) — an operator who navigates away after creating a client without copying the secret has the same problem, and nobody has raised it as a defect. So: **rendering directly does not make the value survive a navigation any better than the "operator copies it from the POST response, it does not survive" option the handoff called the simplest** — it only stops the value from ever becoming a URL, which is D1's actual, narrower requirement. The RFC should say this plainly rather than let a reader assume "render directly" is a durability improvement; it is not, and the handoff's own framing ("may be right") already anticipates this, but the RFC's decision text does not carry the caveat forward.

**A design I would reject, and why:** a short-lived server-side value keyed to the session (the handoff's second candidate) would let a GET reload redisplay the same value once — genuinely different from the render-directly approach — but it adds a store, an expiry, and single-consumption bookkeeping to solve a problem the render-directly approach already avoids for D1's purposes, and which every operator of a secret-issuing admin panel (this RFC's own analogy class — think of any cloud console showing an access key once) is already trained to expect. Not worth building.

## 4. Item 3 — D2's severity, checked rather than assumed

**Confirmed the mechanism; checked, rather than assumed, what it is worth to an attacker.** `clients_edit_get` renders `q.rotated_secret` with no validation (§1). I read how it reaches the page: Leptos's `{sec}` interpolation (`crates/sui-id-web/src/pages/clients.rs:293`) is the same text-interpolation form used for every other field on that page (`{name.clone()}`, `{id.clone()}`), and Leptos escapes text interpolation by default — there is no `inner_html` or raw-HTML sink anywhere in this render path. **This is not an XSS vector.**

**What it is worth: a display-forgery a real attacker gains nothing from.** Nothing the query parameter contains is written anywhere, checked against anything, or granted any access — the *actual* secret is whatever `rotate_client_secret` set in the database, entirely independent of what this parameter shows. The one shape of harm I can construct: an attacker who already knows (or guesses) a client's UUID sends an admin a link — `https://<real-host>/admin/clients/{id}/edit?rotated_secret=<attacker-chosen-string>` — that renders, on the genuine sui-id admin page, under the admin's own authenticated session, a banner claiming this is "the new secret." A social-engineering vector against the admin's own trust in the page's chrome, not a bypass of anything. **The request's own instinct was right: this is untidy, not severe, and I would not rank it above medium on that basis alone** — it is worth fixing because D1 already removes the transport that makes it possible, not because it is independently dangerous.

## 5. Item 4 — D3's placement, and a correction to D3's own reasoning

**Recommend: a router-layer, on named routes — a pattern this codebase already runs, not a new one.** `router.rs:105-112` already attaches `tower_http::set_header::SetResponseHeaderLayer::overriding(CACHE_CONTROL, "no-store")` to `/reset-password`'s whole route (both GET and POST), with a comment stating the rule once ("every response on this route is `no-store`") rather than at each handler. The same mechanism, attached to each of surfaces A–D, E, G's routes in `router.rs`, gives every one of them the header without touching a single handler body.

**Why this beats a response builder or a typed wrapper, concretely.** Both alternatives still rely on a handler *remembering* to reach for them — the exact failure this RFC exists to fix, just with nicer ergonomics once remembered. The router layer does not remove the "a human must remember" risk (a new route can still be added without the layer attached) — no option considered eliminates that — but it *relocates* the remembering into one file, so a reviewer checking "does every secret-bearing route carry this" reads `router.rs` once instead of auditing N handler bodies for a header call that might or might not be there. That is the same win D4 wants from its enumeration, for free, and it is the one property none of the other two options offer.

**Its failure mode, named plainly, per the request's ask:** a new one-time-secret route added to `router.rs` without the layer is silent — nothing forces the pairing. D4's test (§6) is what converts that silence into a build-time failure; the layer alone does not.

**A correction to D3's own text.** D3 bundles two protections under one sentence — "`Cache-Control: no-store` and a referrer policy that does not leak the URL" — as if both matter for the same reason on every surface. They do not. `no-store` protects the **response body** on every one of these six surfaces, unconditionally. A referrer policy protects the **current page's own URL** from being sent as `Referer` when the browser next navigates or loads a resource — which only matters when the secret is *part of that URL*. Once D1 removes the secret from every URL (which it must, regardless), there is no secret-bearing URL left for a referrer policy to protect on **any** of these six surfaces — the global default (`security_headers.rs:131-136`, `strict-origin-when-cross-origin`, applied everywhere already, and not overridden unless a handler sets its own) is already sufficient for a page whose *own* URL never contained a secret. `admin/users.rs`'s `no-referrer` (surface F) is not compensating for a URL-transported secret — F never transported one — it is a second, independent layer of caution the author of that page chose to add, not a requirement D3's own stated reasoning implies for A–E, G. **The RFC should say referrer policy is redundant, not required, once D1 holds** — asking every surface to carry a header whose protection D1 has already made unnecessary is scope the RFC does not need, and stating it plainly is more honest than implying parity with F's belt-and-suspenders choice.

## 6. Item 5 — is D4's list closed and checked, RFC-120-style?

**Not fully, and the RFC should say so rather than imply parity with RFC 120's mechanism.** RFC 120's route enumeration (`tests/e2e/r120_routes.rs`) works because "does this handler take an actor extractor" is a syntactic property of the handler's *signature* — mechanically checkable with no semantic understanding of what the handler does. "Does this handler show a one-time secret" has no equivalent syntactic marker today; nothing about a handler's signature or body distinguishes "renders a `SecretString` the operation just produced" from any other response construction. A fully-derived enumeration, in RFC 120's sense, is not buildable here without first inventing and applying such a marker (a newtype return wrapper, a `#[shows_secret]` attribute macro, or similar) — which is more machinery than this RFC's own "modest exposure" framing justifies building.

**The honest best, and it is still a real, code-checked mechanism, not merely a comment:** a hand-maintained list of the surfaces in §2's table, paired with a test that parses `router.rs` (the same technique `r120_routes.rs` already uses to extract `.route(...)` chains) and asserts that **every route on the hand-list carries the `SetResponseHeaderLayer::overriding(CACHE_CONTROL, "no-store")` layer, and that no *other* route carries it that isn't on the list** — the second half catching drift in the direction RFC 120's own test catches (a route quietly added to or removed from the guarded set is a diff someone must approve, not a silent change). This is weaker than RFC 120's enumeration (the *membership* of the hand-list is not itself derived — a genuinely new one-time-secret surface could still be added without anyone adding it to the list, and nothing catches that half), but it is stronger than "a hand-written list with no check at all," which is what D4's own question offered as the fallback. Recommend building exactly this, and stating the limitation (no mechanical check that the *hand-list itself* is complete) in the RFC rather than leaving a reader to assume otherwise.

## 7. Item 6 — anything else, and is this worth doing at all

**Worth doing, and more clearly so than the RFC's own framing suggests**, precisely because of what the review found rather than what it assumed: surface G is not behind the full admin session every other measured surface is, and it is exactly the kind of endpoint (a machine-to-machine registration API) most likely to sit behind infrastructure — an API gateway, a request-logging proxy — that the RFC's own closing argument names as the future cost of leaving this unfixed. The RFC's "administrative surfaces, a service with no known deployment" framing is accurate for A–F; it undersells G, which this review found and the RFC did not.

**One documentation-accuracy item, unrelated to severity:** the doc comment above `render_qr_svg` in `handlers/admin.rs:65-68` says it is used by `mfa_challenge_post` in `admin/auth.rs` in addition to the enrollment path this review traced. I checked: `render_qr_svg` (private, `:69`) has exactly one call site, `render_qr_svg_pub` (`admin.rs:87`), which is called only from `me_security::mfa_enroll_start`. The comment is stale. Not a security finding, but worth a one-line fix alongside whatever else touches this file, since a reader tracing this exact surface (as this review did) will hit the same dead end.

---

## Findings, ranked

**High**

1. **Surface G — `POST /oauth2/register`'s `client_secret` — is a real, RFC-missed surface with a weaker trust boundary than every other one measured** (§2). No `no-store`; reachable by bearer token, not a full session. Should be added to D4's touches list and D1/D3's closure prerequisites explicitly; it is currently absent from both the RFC and the handoff.
2. **The rotated-client-secret redirect (D1/D2 violation, confirmed) remains the one surface needing a transport change**, not merely a header (§1, §3). The fix is a same-file precedent (`clients_create`'s pattern), not new design.

**Medium**

3. **D2's severity is confirmed non-critical (no XSS, no access granted) — rank it accordingly, and say so in the RFC** (§4), matching the review request's own caution against over-ranking it by adjacency.
4. **D3 conflates two protections with different scopes.** Referrer policy is redundant, not required, on every surface except one that was never affected by the query-string problem in the first place (§5). The RFC should narrow D3's language rather than ask every surface to carry a header D1 makes unnecessary for it.
5. **D4's enumeration is not RFC-120-style derivation, and the RFC should not imply it is** (§6). The buildable version is real and worth building; its limitation (the hand-list's completeness is itself unchecked) should be stated, not left implicit.

**Low**

6. Stale doc comment on `render_qr_svg`'s call sites (§7) — a one-line fix, unrelated to this RFC's substance.

## Recommendation

**Accept with the changes named above.** The core diagnosis (D1–D4) is correct and the fix for the one genuine transport violation (the rotated secret) is a same-file precedent away, not a design problem. Before this ships: add surface G to the touches list and closure prerequisites (finding 1); narrow D3's referrer-policy language to state it is redundant once D1 holds rather than implying every surface needs it for the same reason F does (finding 4); and state D4's enumeration honestly as a hand-list-plus-router-check, not a full derivation (finding 5). None of these change what needs to be built — they change what the RFC claims about what it built, which is exactly the standard this whole line of reviews has been holding every other RFC to.

**Entry point of this package:** `.git-exclude/review-requests/rfc-122-design-review-2026-09-30.md`
