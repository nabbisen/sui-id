# RFC 095 metadata-validation matrix

This matrix is normative. “Reject” means before token consumption.

## Envelope and member policy

| Input | Result |
|---|---|
| JSON object, at most 64 KiB, depth <=16, top-level members <=128 | Continue |
| Non-object, malformed JSON, duplicate member | `invalid_client_metadata` |
| Unknown extension member | Ignore; do not persist/echo |
| Known unsupported RFC/OIDC member | `invalid_client_metadata` |
| `software_statement` | `unapproved_software_statement` |
| Wrong JSON scalar/array type | `invalid_client_metadata` |

Bearer input is exactly one 64-character lowercase hexadecimal token. Missing,
oversized, non-hex, uppercase, multiple, or wrong-scheme authorization values
take the same public invalid-token path.

## Supported fields

| Field | Valid | Invalid |
|---|---|---|
| `redirect_uris` | 1–16 unique redirects in one derived closed profile | absent, empty, duplicate, invalid/mixed-profile element, >16 |
| `client_name` | trimmed 1–128 scalars, <=512 bytes | absent/blank, control/bidi override, oversize |
| `token_endpoint_auth_method` | none/basic/post; default basic | any other string |
| `grant_types` | code; code+refresh; default code | empty, duplicate, refresh without code, other grant |
| `response_types` | code; default code | empty, duplicate, token/hybrid/other |
| `scope` | 1–32 unique catalog tokens including openid; **required — no default** | **absent**, empty, malformed, unknown, duplicate, oversize |
| `post_logout_redirect_uris` | absent/empty or 1–16 canonical HTTPS unique redirects, independent of authorization profile | HTTP (including loopback), duplicate, invalid element, >16 |
| application URI fields | absent or one canonical HTTPS URI | HTTP, userinfo, fragment, noncanonical, oversize |

`refresh_token` requires `offline_access`; `offline_access` requires
`refresh_token`.

> **`scope`'s absent case, stated 2026-10-20.** Every other row above is
> explicit about absence — `redirect_uris` and `client_name` list `absent` as
> invalid; three rows give a default; two say "absent or …". This row said
> nothing, and M3 stage 2 had to resolve the silence to implement it. The
> rule is **required**: the Valid column demands `openid` *among the tokens*,
> which an absent `scope` cannot satisfy, and defaulting to `openid` would
> grant a scope the registrant never asked for.
>
> **This is a deliberate departure from RFC 7591 §2**, where `scope` is
> OPTIONAL in a client registration request. A server may refuse metadata on
> policy grounds, so it is within spec — but an RFC 7591 client library that
> omits `scope` will be refused, and **the integrator-facing documentation
> must say so** (stage 3). The ambiguity was the architect's; it is fixed
> here rather than left for each stage to re-decide.

## Redirect corpus

| Case | Expected |
|---|---|
| Basic/POST + `https://rp.example/cb` | Accept as `ConfidentialHttps` |
| none + `https://rp.example/cb` | Accept as `PublicHttps`; PKCE required |
| HTTPS with canonical path/query | Accept; exact query/order is registered |
| none + `http://127.0.0.1:49152/cb` | Accept as `PublicNativeLoopback`; PKCE required |
| none + `http://[::1]:49152/cb` | Accept as `PublicNativeLoopback`; PKCE required |
| Basic/POST + any HTTP loopback | Reject |
| none + mixed HTTPS and HTTP loopback list | Reject |
| Same numeric-loopback authorization URI with different request-time port | Match only for `PublicNativeLoopback` |
| Any HTTP post-logout URI, including numeric loopback | Reject for every dynamic profile |
| Public-native authorization redirects plus independent HTTPS logout URI | Accept; authorization profile unchanged |
| HTTPS URI differing by case/path/query/encoding | No match |
| Loopback URI differing by path/query/IP family | No match |
| `http://localhost:49152/cb` | Reject |
| HTTP remote/private host | Reject |
| Loopback without explicit port or with port zero | Reject |
| fragment or userinfo | Reject |
| wildcard/prefix/suffix pattern | Reject |
| custom scheme, `file`, `data`, `javascript` | Reject |
| Unicode hostname rather than canonical ASCII punycode | Reject |
| canonical ASCII punycode HTTPS hostname | Accept |
| malformed percent escape/backslash/default-port alias | Reject |
| exact duplicate in one list | Reject |
| URI >2,048 bytes or list >16 | Reject |

The runtime matcher uses exactly the same parser/canonicality predicate as
registration. Separate “validation” and “matching” URL interpretations are
forbidden.

For dynamic logout, exact registered HTTPS matching is necessary but
insufficient. A verified `id_token_hint` must bind to the same client; an active
session must also agree on subject and `sid` when present. An unexpired valid
hint may establish RP legitimacy without a session. An expired hint is accepted
only with a matching active session and `now - exp` from 0 through 300 seconds
inclusive; 301 seconds is rejected. The grace is logout-only.

GET query and form-encoded POST use the same bounded, duplicate-rejecting
parser. Missing, invalid, over-grace, client-mismatched, or session-mismatched
hints leave the session unchanged, suppress every redirect/fallback, and render
only local confirmation/error. Local logout then requires an explicit
single-use, short-lived, session/purpose-bound CSRF token in a subsequent POST;
invalid CSRF leaves the session unchanged and confirmed local logout ignores
all RP parameters.

## Name and scope corpus

- Name tests include leading/trailing whitespace, blank, 128/129 scalars,
  512/513 bytes, combining characters, emoji, C0/C1 controls, bidi override and
  isolate characters, HTML, quotes, and log delimiters. Valid values are
  escaped by the UI and bounded/sanitized before audit.
- Scope tests cover every allowed RFC 6749 scope-token character, space,
  quote/backslash exclusions, duplicate tokens, unknown catalog values,
  concurrent catalog deletion/default change, 32/33 tokens, 64/65-byte token,
  and 1,024/1,025-byte total.

## Authentication/grant matrix

| Auth method | Header credentials | Body credentials | Result |
|---|---|---|---|
| none | absent | client ID only, no secret | Accept with PKCE |
| none | Basic or any secret | any | Reject |
| basic | exactly one Basic pair | absent | Accept |
| basic | absent | secret in body | Reject |
| post | absent | client ID and secret in body | Accept |
| post | Basic present | any | Reject |
| legacy any | Basic or body, not both | compatibility only | Accept for backfilled/admin legacy |
| any | Basic and body both present | both | Reject |

Each row is tested for authorization-code and refresh dispatch. Refresh is
rejected unless both grant policy and scope policy authorize it.

## Browser presentation

- A dynamic `logo_uri` is stored/echoed but never rendered or fetched by
  consent or other browser UI.
- Dynamic application links require a user action and use
  `rel="noopener noreferrer"` plus no-referrer policy.
- ~~A browser-network test must observe no request to a registrant-controlled
  logo origin while displaying dynamic-client consent.~~ **Superseded
  2026-10-21, before stage 5 was dispatched.** See the note below.

> **The third bullet was a verification method, and it was the wrong one.**
> It named a *browser* observation; RFC 095 `:269-270` places the obligation
> on the server — *"Consent and other browser UI must not **emit** an image
> or other automatic request for a dynamically registered `logo_uri`"* — and
> `:572` states the criterion as *"Dynamically supplied logo URIs cause no
> automatic server or browser request."*
>
> **Asserting the URL is absent from the rendered HTML tests the rule
> itself**; a browser observation tests a consequence of it through a third
> party's implementation, non-deterministically, and can pass for the wrong
> reason. The workspace has no browser driver — measured 2026-10-21, zero
> hits for playwright, webdriver, chromedriver, fantoccini, thirtyfour and
> headless Chrome across every manifest, workflow and script — and adding one
> to an IdP for a property provable from a string is a supply-chain and CI
> cost with no evidence gain.
>
> **Replaced by:** a rendered-HTML assertion that a dynamic client's
> `logo_uri` appears nowhere in the consent response, paired with an
> admin-client control proving the logo is not simply dropped for everyone,
> and an end-to-end case through `/oauth2/authorize`.
>
> **This departs from this document, not from RFC 095**, which is unchanged
> and needs no amendment. The gloss was the architect's.

## Response/error assertions

- Success echoes every registered supported value after defaults and canonical
  ordering, plus required issued identifiers/secret timestamps.
- Ignored unknown metadata is absent.
- Invalid-token states have identical status, `WWW-Authenticate`, body, and
  cache headers.
- Public errors contain only fixed ASCII templates and field/index identifiers.
- Every success/error has `Cache-Control: no-store` and `Pragma: no-cache`.
