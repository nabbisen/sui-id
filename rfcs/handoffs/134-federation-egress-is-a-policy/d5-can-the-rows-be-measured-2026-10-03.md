# D5 — can the five rows be calculated from real-world reality and its needs?

**Date:** 2026-10-03. **Question asked by `@nabbisen`.**
**Status: analysis only. Not adopted, and RFC 134 is unchanged.** Adopting it is
a material change to scope and prerequisites, which under RFC 000 returns an
Accepted RFC to `proposed/`. That is his call, not mine, and the cost is stated
at the end.

## Short answer

**Yes — and the question is better than the framing I gave you.** I offered two
options, literal compliance or amend. Measuring reality produces a third that is
better than either, and it shrinks the problem: **three of the five rows stop
needing an amendment at all.**

I had split the rows by *difficulty*. They should be split by **enforceability**,
which is a property of the stack rather than an opinion.

## Tier 1 — ours already; derive the number from a measured corpus

These are enforced in our own code, not the library's. Nothing blocks a measured
bound today.

| Row | Where we enforce it |
|---|---|
| Body byte cap | we read the body; `Response::content_length()` pre-check plus a hard cap on bytes actually read |
| JSON depth / member / string / array caps, duplicate keys | our deserializer |
| Media type and status | our code, after the response returns |

**The measurement.** Fetch the discovery document, JWKS and userinfo shape from a
corpus of real providers — Google, Microsoft Entra, Okta, Auth0, Keycloak,
GitLab, Authentik — record the observed maxima, and set each bound at observed
maximum × a stated headroom. Record the corpus, the date, and each number's
derivation next to the constant.

**This is strictly better than both of my options.** A number derived from a
measured maximum has a reason attached; the previous architect's numbers may well
be sensible but nobody can say why they are those numbers, and a bound nobody can
justify is the one that gets raised the first time something legitimate trips it.

## Tier 2 — already strict, and we can stop *relying* on that

This is the part I got wrong, and it dissolves the cost I warned you about.

reqwest 0.13.4 exposes three opt-in laxity toggles:

- `http1_allow_obsolete_multiline_headers_in_responses`
- `http1_ignore_invalid_headers_in_responses`
- `http1_allow_spaces_after_header_name_in_responses`

They are named to be opted *into*, so strictness is the default. **But the fix is
not to rely on that — it is to call all three explicitly with `false`.**

Doing so converts "we inherit a default" into "we require this", in our own
source. **That removes the exact risk I told you was the cost of amending:** a
future reqwest or hyper release relaxing a default can no longer reach us
silently, because our code states the requirement. It also means I never have to
verify what the default is, which is the stronger position — I was about to go
and confirm three defaults I can simply assert instead.

So the "bare LF / obs-fold / invalid header" row is **satisfiable today, by us,
with no amendment.**

## Tier 3 — genuinely out of reach, and only here does an amendment apply

reqwest does not expose hyper's header-slot or buffer sizing, and nothing exposes
the TLS handshake byte bound or "connection dropped without EOF wait":

- fixed 32 KiB header buffer, 64 slots, 8 KiB scratch
- handshake ≤256 KiB inbound
- connection dropped without EOF wait

**Certificate chain ≤16/128 KiB is uncertain, not settled:** `tls_info` may expose
enough to check it post-handshake. I have not verified that, and I am not going
to claim it either way — it is the one row that needs a measurement before it can
be filed in Tier 2 or Tier 3.

## What this changes

| | Before | After |
|---|---|---|
| Rows needing a matrix amendment | 5 | **2, possibly 3** |
| Numbers justified by a recorded measurement | 0 | every Tier 1 bound |
| Reliance on an upstream default | the stated cost of amending | **removed** — Tier 2 states the requirement in our source |

## The cost of adopting this

**RFC 134 returns to `proposed/`.** This changes scope and closure prerequisites,
which RFC 000 line 43 makes a material change: the acceptance metadata comes off,
the index and inbound links update, and the RFC needs re-acceptance. That is real
process cost one day after accepting it.

**My recommendation: accept that cost.** The design is better, the work is not yet
dispatched so nothing is wasted, and returning an RFC for a genuine improvement is
what the return rule is *for* — it is not a penalty. The alternative is editing an
Accepted RFC's scope in place, which is the thing RFC 000 exists to prevent and
which I would flag immediately in anyone else.

**If you would rather not reopen it**, the honest fallback is to carry Tier 1 and
Tier 2 as a follow-on RFC that amends 134 rather than editing it, and leave 134
as accepted. That is more documents for the same outcome, and I would not choose
it, but it is legitimate.
