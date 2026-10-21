# M3 stage 5 — browser presentation

> ## Whose turn it is: **the implementation role's.**
> Nothing in this stage waits on `@nabbisen`. **Stage 4 does** — a ruling that
> is not the architect's to make — so stage 5 is dispatched ahead of it. The
> reorder is §0.

**Dispatched.** 2026-10-21 JST, by the architect.
**Plan.** [`m3-stage-plan-2026-10-20.md`](m3-stage-plan-2026-10-20.md) §C,
stage 5 — **taken out of order**, see §0.
**Normative source.** **RFC 095 `:266-272`**, not the matrix summary. The
difference matters and §1 is about exactly that.
**Stage 3's review.** `.git-exclude/reviewed/rfc-095-stage3-response-2026-10-21.md`.
**Baseline.** Read the tip with `git log -1`.

## 0 — why this comes before stage 4

Stage 4 is blocked on one question that belongs to `@nabbisen` and to nobody
else — who may perform a design review here — recorded in
[`stage4-design-the-authentication-grant-matrix-2026-10-21.md`](stage4-design-the-authentication-grant-matrix-2026-10-21.md)
§E. Holding stage 5 behind it would idle this role for a decision stage 5
does not depend on, and the standing constraint is that process must not be
charged where it buys nothing.

**Stage 5 needs no design review of its own.** Stage 4 was singled out in the
plan as *"the one security-bearing stage here"* because a wrong accept there
is an authentication bypass. Stage 5 has no comparable latitude: RFC 095
states the rule in prescriptive terms and this dispatch quotes it. There is
no design to review — only a rule to meet and evidence to produce.

Ordering is now 2 → 3 → **5** → 4 → 6. Nothing else in the plan changes.

## 1 — the rule is about *emitting*, not about observing a packet

My own [`metadata-validation.md`](metadata-validation.md) §*Browser
presentation* says:

> *"A browser-network test must observe no request to a registrant-controlled
> logo origin while displaying dynamic-client consent."*

**That is a verification method I wrote, and it is not what the RFC
requires.** RFC 095 `:269-270`:

> *"Consent and other browser UI **must not emit an image or other automatic
> request** for a dynamically registered `logo_uri`; the value is retained
> only for protocol/operator inspection."*

and its acceptance criterion, `:572`:

> *"Dynamically supplied logo URIs cause no automatic server or browser
> request."*

The normative obligation is on **what the server emits into the HTML**. A
browser observation watches a *consequence* of that obligation through a
third party's implementation. Asserting the element is absent tests the rule
itself, is deterministic, and cannot pass for the wrong reason.

**So: no browser driver.** The workspace has none today — I grepped for
playwright, webdriver, chromedriver, fantoccini, thirtyfour and headless
Chrome across every manifest, workflow and script and found **zero** hits.
Adding one to an IdP for a property provable from a string is a supply-chain
and CI cost with no evidence gain.

**This is a departure from a handoff document and it is recorded as one**, in
`metadata-validation.md`, the same way stage 2's required-`scope` departure
was. The RFC is unchanged and needs no amendment: the departure is from my
gloss, not from RFC 095.

## 2 — what is actually there today, measured

I traced this before writing it, and the answer is not "unbuilt".

**The consent screen renders a logo for every client, with no provenance
check.** `sui-id-web/src/pages/oidc.rs:63-69`:

```rust
{logo_uri.map(|uri| view! {
    <div class="consent-app-logo">
        <img src=uri alt=client_name_display.clone()
             class="consent-app-logo__img" />
        <p class="text-caption muted">{"App logo provided by the application"}</p>
    </div>
})}
```

and the handler that feeds it, `sui-id/src/http/handlers/oidc.rs:211`:

```rust
logo_uri: client_for_consent.logo_uri.clone(),
```

`client_for_consent` is the whole `ClientRow` (`:154`), so
`registered_via: RegistrationSource` (`sui-id-store/src/models.rs:313`) is
**right there and unread**. A dynamically registered client's `logo_uri` is
emitted as an `<img src>` into the consent page. **RFC 095 `:269` is
violated at the emit level, today.**

**Why no request is made anyway, and why that is not a defence.** The global
security-headers middleware (`sui-id/src/http/security_headers.rs:120-123`,
layered over every route at `router.rs:417-427`) sets

```
img-src 'self' data:;
```

so a conforming browser refuses to load the cross-origin image. **I checked
that no handler sets its own `Content-Security-Policy`** — the middleware
only fills the header in when absent, and in non-test code it is the sole
setter — so the block holds on every deployment sui-id controls.

That means **this is not an exploitable hole and I am not treating it as
one.** It is the rule being met by a header that the code emitting the
element knows nothing about, with no test joining them, one operator-supplied
CSP away from not being met. RFC 095 did not ask for a mitigated emit; it
asked for no emit.

**Two things follow that the plan did not anticipate, and §3 and §6 are
them.**

## 3 — the feature is dead for admin clients too, and the screen says otherwise

`img-src 'self' data:` does not distinguish clients. **An operator-registered
client's logo is blocked by exactly the same rule.** RFC 008 added
`logo_uri`, `homepage_uri`, `privacy_policy_uri` and `tos_uri` so the consent
screen could show who is asking; measured today, the logo half of that has
never worked in a CSP-respecting browser.

What the user sees is a broken image, and beneath it, unconditionally:

```
"App logo provided by the application"
```

**A caption asserting a logo was provided, under a hole where the logo
isn't.** That is the second half of the project's standard — an interface a
user cannot misread — failing in the plainest way.

**Do not fix this by loosening the CSP.** Allowing arbitrary remote image
origins on the consent screen is the thing RFC 095 `:269` exists to prevent,
and it would hand every admin-registered client the same capability. Options
that do not loosen it (proxy-and-cache the logo through sui-id's own origin;
store it as a `data:` URI at registration; drop the feature) differ in cost
and in what they commit the operator to.

**This is not stage 5's to settle and you must not settle it.** Report it as
a finding with your measurements. I will put it to `@nabbisen`. RFC 008's
logo feature is his to repair, scope down or withdraw; that is a product
question, not an implementation one.

**What stage 5 does do** is stop the caption lying: the caption and the
`<div>` must not render when no image will. Today they render whenever
`logo_uri` is `Some`.

## 4 — what to build

### 4.1 — the provenance gate, in the renderer

`ConsentData` gains the provenance, typed:

```rust
pub registered_via: sui_id_store::models::RegistrationSource,
```

`sui-id-web` already depends on `sui-id-store` (`Cargo.toml:16`), so this
needs no new dependency and **must not be a `bool`** — a bare flag at a call
site is the kind of thing that gets passed inverted and nothing notices.

`render_consent` suppresses the logo block entirely when it is
`RegistrationSource::Dynamic`: no `<img>`, no wrapping `<div>`, no caption.

**The gate goes in the renderer, not the handler.** The obligation is on what
is emitted, so it belongs with the code that emits it; put it in the handler
and the next caller of `render_consent` re-breaks it silently. The handler's
job is only to pass `client_for_consent.registered_via` through.

### 4.2 — the application link stays, and gets pinned

RFC 095 `:266-272` forbids *automatic* requests. A link the user clicks is
not automatic, and the matrix allows it on conditions. **All three hold
today** — do not change them, **test them**, because nothing does:

- it requires a user action — it is an `<a href>`, `oidc.rs:75`;
- `rel="noopener noreferrer"` — `oidc.rs:75`;
- the referrer policy — `AuthShell` emits
  `<meta name="referrer" content="same-origin" />` (`layout.rs:114`), which
  sends **no** `Referer` on a cross-origin navigation. Note it is
  `same-origin`, not the literal `no-referrer` the matrix names; the
  cross-origin behaviour is identical and `same-origin` is what the rest of
  the UI uses. Say so in the test's name or a comment so the next reader does
  not "fix" it to match the matrix's wording.

Check `homepage_uri` is the only such link on this screen before you assert
that. I believe it is; measure it.

## 5 — evidence

**A rendered-HTML assertion, not a browser.** `render_consent` returns
`String`, so a unit test in `sui-id-web` can call it directly. Per the
project's convention put it in a sibling file — `pages/oidc/tests.rs` with
`#[cfg(test)] mod tests;` in `pages/oidc.rs`, as `pages/clients.rs:424-425`
does.

Required, and each must be able to fail:

1. **Dynamic, logo set → the URL does not appear in the output at all.**
   Assert on the URL string, not on `<img`. Use a distinctive host so a
   substring match means what it says.
2. **Dynamic, logo set → the caption string and the `consent-app-logo`
   wrapper are absent.** §3's half.
3. **Admin, logo set → the URL does appear.** Without this the first test
   passes if the logo is dropped for everyone, which is a different bug that
   would look identical.
4. **The link properties**, from §4.2 — `rel` and the `meta` — asserted on
   the real rendered string.

**End to end as well.** `tests/e2e/r120.rs:117` already has a `consent_screen`
helper that drives `GET /oauth2/authorize` to an HTML consent response; reuse
it rather than building a second harness. One e2e: register a client through
`/oauth2/register` with a distinctive `logo_uri` host, reach consent as that
client, assert the host appears nowhere in the body. That closes the gap
between "the renderer is correct" and "the handler passes the right thing".

**Mutation evidence, for test 1 only.** Remove the provenance gate and show
test 1 failing with the host in the output. One mutation. This project has
twice shipped a test that passed while testing nothing — RFC 107's binding
test and the `json_shape` depth test — and both times the mutation was the
thing that would have caught it.

**Also report, measured, not argued:** whether `Content-Security-Policy`
reaches the consent response in the e2e, and its exact value. I read it from
the middleware; I did not observe it on that route. If it is not there, say
so — that changes §2's conclusion and I want to know before I write it to
`@nabbisen`.

## 6 — three stale comments, each saying the opposite of the truth

Same class as stage 3's finding: prose no gate checks, left behind.

- **`security_headers.rs:21-24`** — *"`frame-ancestors 'none'` makes
  clickjacking on `/oauth2/authorize` impossible — **sui-id has no consent
  screen**, but…"*. It does; `render_consent` is reachable from
  `/oauth2/authorize` whenever `consent_policy` asks. The conclusion stands;
  the reason given for it is false.
- **`sui-id-web/src/pages/oidc.rs:10`** — `/// RFC 008: optional application
  logo URL (validated HTTPS, **never fetched**).` It is emitted as `<img
  src>`; "never fetched" is true only because of a CSP header in another
  crate. After §4.1 it becomes true for dynamic clients by construction —
  say *that*, and name the CSP for the admin case.
- **`sui-id-store/src/models.rs:314`** — `/// RFC 008: application identity
  URLs (validated HTTPS, never fetched).` The same false clause, one crate
  over. Same correction.

Fix all three. Do not widen the sweep beyond them.

## 7 — documentation

`docs/src/reference/oidc-api.md` §*Dynamic client registration (RFC 7591)*
already shows `logo_uri` in an example request (`:81`). An integrator reading
that will reasonably expect it to be shown to users. **State that a
dynamically registered `logo_uri` is stored and echoed for operator
inspection and is never displayed**, with the reason in one clause. This is
integrator-visible behaviour they cannot discover by testing — their logo
simply never appears — and that is precisely the kind of silence the project
treats as a defect.

## 8 — what to return

The usual package, plus:

- **The §5 CSP observation**, with the header value as the test saw it.
- **Your §3 measurements** — confirm or correct my reading that an
  admin-registered logo is blocked too, and say how you established it.
  Report it; do not act on it.
- **Whether `homepage_uri` is the only user-activated external link** on the
  consent screen.
- Mutation evidence for test 1.
- The full matrix; Level B scope. Report it local.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the last commit
  of the round, baselined on the tip at that moment.

**A `CHANGELOG` entry: yes.** `### Fixed` — a dynamic client's `logo_uri` was
emitted into the consent page. **Not `### Security`**: no request is made
today, because the CSP blocks it, and an entry claiming otherwise would be
wrong in the direction that matters. Lead with what changes for an operator:
a dynamic client's logo block no longer renders. If your §5 measurement shows
the CSP is *not* on that response, stop and return it to me before writing
any entry — that is a different document and a different severity, and
`.github/SECURITY.md` reserves the severity call to `@nabbisen`.

## 9 — what this stage does not do

Not the authentication/grant matrix (stage 4, blocked on a ruling), not the
legacy-candidate report (stage 6), **not a decision about RFC 008's logo
feature** (§3 — report only), and not a CSP change of any kind. It does not
close M3.

Leave the working tree uncommitted. One open dispatch at a time; this is it.
