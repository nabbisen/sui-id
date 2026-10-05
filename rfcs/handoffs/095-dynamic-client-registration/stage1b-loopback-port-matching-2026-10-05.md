# RFC 095 M3 stage 1b — the native-loopback port exception

**RFC status: Accepted.** Entry gate open.

**Dispatched.** It completes the one redirect-corpus row stage 1 deferred.

## I put this on `@nabbisen`'s desk and it does not belong there

I said this needed his decision — whether to add port-flexible matching to
`is_redirect_uri_registered` against its standing directive, or carry the row as
an exception. **Both options were wrong, because the question is already
settled**, in RFC 095's own accepted architecture:

> [`architecture.md:197-199`](architecture.md) — *"CORS origin comparison always
> includes the exact port. The RFC 8252 variable-port exception belongs only to
> `PublicNativeLoopback` authorization redirect matching and never applies to
> logout or CORS."*

That is the policy, decided, with its scope already drawn exactly where it
should be. **What was missing is the mechanism, not the decision.** I escalated
because I had not read the architecture document of the RFC I was dispatching —
the same failure as RFC 134's "admin UI" and closure criterion 5, and now the
fifth instance.

**If `@nabbisen` reads `architecture.md` as not settling it, stop and say so**
and this waits. But an Accepted RFC's architecture naming the exception, its
sole applicable profile, and the two places it must *not* reach is a decision,
not a suggestion.

## Why it is required, not a convenience

A native app using a loopback redirect **cannot know its port in advance** — the
OS assigns an ephemeral one at launch. Register `http://127.0.0.1:49152/cb`,
get port 51234 next run, and exact matching fails every time. **Without this,
`PublicNativeLoopback` is a profile that can be registered and never used**, and
stage 1 built the registration half of it.

RFC 8252 is already cited three times in this project, including by RFC 095
itself (`:48`) for exactly this.

## The mechanism — and it needs no schema change

**Do not touch `is_redirect_uri_registered`.** Its body is
`registered.iter().any(|u| u == submitted)` under *"Keep it boring; resist any
urge to add normalisation in here"*, two lines below a paragraph about
attacker-controlled callbacks. It is used by **every** client; a change there
widens matching for all of them. The directive is right and stays honoured.

**The profile is re-derivable at request time from data already stored.**
`RedirectProfile` is **not persisted** — it lives only in the validation module
— but its two inputs are: `ClientRow.redirect_uris` and
`ClientRow.confidential` (`models.rs:296,298`), and profile derivation needs
only `is_public`, which is `!confidential`. **So no migration, no new column.**

At `authorize.rs:86` and `:148`, the shape is: derive the profile from the
stored row; if and only if it is `PublicNativeLoopback`, use a loopback-aware
comparison; otherwise call the existing exact matcher unchanged.

**The loopback-aware comparison differs from exact equality in one respect and
no other: the port.** Scheme, host, path, query and fragment must match exactly.
A difference in any of them is a rejection, as today.

## What must not acquire port flexibility

Stated because the architecture states it, and because this is where the bug
would be:

- **logout** (`post_logout_redirect_uris`) — never, for any profile;
- **CORS** origin comparison — never, for any profile;
- **every profile other than `PublicNativeLoopback`** — a `ConfidentialHttps`
  or `PublicHttps` client gets exact matching, and a client whose stored
  redirects are HTTPS can never derive the loopback profile anyway.

## Tests

- A `PublicNativeLoopback` client: registered `http://127.0.0.1:49152/cb`,
  request-time `http://127.0.0.1:51234/cb` → **match**.
- The same client, request-time differing in **path**, **query**, **host**
  (`[::1]` vs `127.0.0.1`), or **scheme** → **no match**. Port is the only
  degree of freedom.
- A `ConfidentialHttps` and a `PublicHttps` client with a port-differing
  request → **no match**. Build these by storing the right row shape, so the
  test proves the profile gate and not just the comparison.
- **Logout and CORS are unaffected**: a port-differing post-logout URI is
  rejected for a `PublicNativeLoopback` client, and the CORS origin check still
  includes the port for one.
- The removal check: disable the profile gate so every client gets the
  loopback-aware path, and confirm the `ConfidentialHttps` port test fails.

## Return

Per-hunk SHA-256 against a stated baseline, the removal evidence, and the full
local gate set — **including A3.2, G19 and G20**.
