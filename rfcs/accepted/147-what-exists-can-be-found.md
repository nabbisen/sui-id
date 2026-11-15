# RFC 147 — What already exists can be found, and says what to do next

**Status.** Accepted
**Accepted on.** 2026-11-15
**Approved by.** `@nabbisen`, 2026-11-15: *"All accepted."*
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** This RFC Accepted. Under the
operator-experience track, **nothing in the track is built until its
schedule and every one of its RFCs exist** (ROADMAP, *Active plan*, decided
2026-11-15).
**Closure prerequisites.** Each of §1–§5 is built, and has the test it names,
which fails when that fix is reverted. Each test is mutated to show it can
fail, and the mutation recorded.
**Tracks.** The operator-experience track, Track A (repairs). These are
repairs under the feature freeze's exception, as decided 2026-11-15.
**Touches.** `crates/sui-id-web/src/pages/{users,clients,signing_keys,federation_providers}.rs`,
`crates/sui-id-i18n/src/locale/*.rs`, `crates/sui-id/src/main.rs`,
`crates/sui-id/src/runtime/config.rs`, `sui-id.example.toml`.
**Independent design review.** [Design review record, 2026-11-15](../handoffs/147-what-exists-can-be-found/design-review-2026-11-15.md). By `@nabbisen`, who did not author it.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/147-what-exists-can-be-found/README.md`](../handoffs/147-what-exists-can-be-found/README.md)

## Summary

Five things sui-id already does that an operator cannot find, or that stop
without saying what to do. Each was found when the owner used the admin
console and the first run on 2026-11-15, and each was measured at
`file:line` on `7c35c42`. **None is new behaviour.** Each makes an existing
function visible, or makes an existing stop say the next step.

## Why this is an RFC

They change user-facing contracts: what a page offers, and what an error
says. RFC 000 asks for an RFC for that, whatever the size. They are grouped
because they share one cause. Each was built correct, and was then never seen
by a newcomer.

## Scope, and what is elsewhere

| Raised | Where it is decided |
|---|---|
| Sign-out beside *Settings* on 38 of 39 pages | **RFC 144**, refresh of 2026-11-15 (the account menu on every page) |
| `setup` missing from `--help`'s usage | **RFC 107**, which is accepted: *"Every subcommand the binary dispatches appears in `--help`"* |
| The command structure, `init`, `sui-id help`, sample profiles | **RFC 148** |
| Navigation, help policy, list editors, the palette | **RFC 149** |
| Editing users, password policy, adding a provider in the UI | Track D: one RFC each, each with its own freeze exception |

## Decision

### 1 — every primary action has a visible label

Three list pages offer their main action as an icon-only button. Its name
is in `aria-label` only, so a sighted reader sees a bare symbol:

| Page | Action | Measured |
|---|---|---|
| Users | *Add user* | `"+"`, `pages/users.rs:90-93` |
| Apps | *Add app* | `"+"`, `pages/clients.rs:140-142` |
| Signing keys | *Rotate the signing key* | icon only, `pages/signing_keys.rs:89-90` |

**Decision.** Each becomes a button with its words visible, using the label
the `aria-label` already carries, so no new translation string is needed.
An icon may stay beside the words.
**Test.** No `<a>` or `<button>` in `crates/sui-id-web/src/pages` has a
`button--icon` class without visible text. **Mutation:** restore the `"+"`.

### 2 — an empty page says how to fill it

| Page | Says today | Measured |
|---|---|---|
| Federation providers | *"No federation providers are configured yet."*, and nothing else | `federation_providers.rs:144`, `None` for the action |
| Signing keys | *"No signing keys found."* | `signing_keys.rs:103`, `None` |

A provider is added only through a `[[federation_providers]]` block and a
restart. The page should say so, and link to the configuration reference.
Signing keys should not be empty at all, because one is created at first
start. If they are, the page should say what that means and what to do.
**Decision.** Both empty states name the next step. The federation one also
links to `docs/src/reference/configuration.md`'s `[[federation_providers]]`
section, at the URL the deployed docs use. The audit log's empty state
(`audit.rs:120`) stays as it is: an empty log has no next step.
**Test.** Every `empty_state(…, None)` call is on a short list in the test,
each with a written reason. Today that list holds only the audit log. A new
`None` fails until someone gives it a reason.

### 3 — a missing configuration says how to make one

Measured from an empty directory, on the `0.91.0` binary:

```text
$ sui-id
Error: loading config from ./sui-id.toml

Caused by:
    failed to read config ./sui-id.toml: No such file or directory (os error 2)
```

`sui-id setup --admin-username admin` fails the same way. A user who
downloaded the binary, the documented install since `0.89.0`, has **no
`sui-id.example.toml`**: the release ships only the binaries and
`SHA256SUMS`.

**Decision.** When the configuration file does not exist, whether at the
default path or a `--config` path, every command that needs it says, in
order:
1. that the file does not exist, and its path;
2. how to create one: `sui-id --print-sample-config > sui-id.toml`, a
   local-trial configuration;
3. where the production template and the reference are:
   `sui-id.example.toml` in a source checkout, and the configuration
   reference.

A file that exists but cannot be read, or does not parse, keeps its
current message. Those are different problems.
**Test.** An e2e test runs the binary in an empty directory, once with no
argument, once with `setup`, and once with `--config ./missing.toml`. It
asserts each of the three parts. **Mutation:** restore the bare error.

### 4 — the production template names every key the configuration accepts

`sui-id.example.toml` (119 lines, commented, production-oriented) and
`--print-sample-config` (generated from `Config::sample()`,
`runtime/config.rs:239`, local-trial) are **two profiles, and that is
right**. But the template **omits keys the configuration accepts**: the
printed sample has `user_sources`, `federation_providers`,
`metrics_enabled`, `metrics_listen_addr` and `access_log`, and the template
mentions none of them. RFC 148 decides how a newcomer is offered either
profile. **This RFC fixes only the omission.**

**Decision.** The template carries every key of `Config::sample()`, either
set or commented with its default, under its section.
**Test.** A test parses `sui-id.example.toml` as a `Config`, and checks that
every key `Config::sample()` serializes appears in the template, set or
commented. **Mutation:** delete one key from the template.

### 5 — a URL field is a URL field, and creating an app offers what editing does

- `homepage_uri`, `privacy_policy_uri` and `tos_uri` are `type="text"`
  (`pages/clients.rs:440-448`). They become `type="url"`, which gives a
  phone's URL keyboard and the browser's own check. The server's
  validation is unchanged, and remains the authority.
- **Creating an app has no *Consent policy* field**, but editing one does
  (`clients.rs:417`). An app is therefore created with a policy its creator
  never saw. **Decision:** the create form gains the same field, with the
  same default, shown.
- **Test.** A rendering test asserts both forms carry the same set of
  fields, except those that exist only after creation, listed in the test
  with a reason. **Mutation:** remove the field from the create form.

## What this RFC does not do

It adds no function. **It does not decide structure**: where an action
should live, or how the navigation is grouped, is RFC 149's. And the
scopes and redirect-URI fields remain text, because a list editor is a
form-system design, which is RFC 149's.

## Alternatives rejected

- **Fold these into RFC 149.** Rejected. RFC 149 is design-first, with
  mockups, and will take time. These are defects in what exists today, and
  the freeze lets them be repaired now, as decided 2026-11-15.
- **One sample configuration instead of two.** Rejected. A local trial and
  a production deployment need different defaults (`127.0.0.1` against an
  HTTPS issuer; `cookie_secure` off against on). Merging them would make one
  of them wrong. The defect is the omission, not the existence of two.
