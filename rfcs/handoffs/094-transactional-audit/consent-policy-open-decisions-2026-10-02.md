# Two open decisions on dynamic-client consent

**Date:** 2026-10-02
**Status: both questions are open.** Nothing here is dispatched; no code should
anticipate either answer, and the dev team has been told so. Addressed to the
project owner; I give a recommendation for each and neither is settled.

**Raised by:** the `consent_policy` persistence fix (`b35adc0`), which closed the
bug but left both questions unanswered.
**Standard applied, his:** *"finally clean, safe and secure, and robust and
sophisticated design"*, and *"APIs and UI/UX for users not to be confused or
misunderstand are also very important."*

## Q1 — is `FirstTime` the right policy for a dynamically registered client?

**Recommendation: yes, keep it. The defect is that no RFC says so, not the value
itself.**

`crates/sui-id-store/src/models.rs:211-219` defines three policies, and the
doc comments carry the reasoning:

| Policy | Meaning, verbatim |
|---|---|
| `None` (the `#[default]`) | "No consent screen — **first-party default**. Existing behaviour." |
| `FirstTime` | "Show consent on first authorization; skip if prior grant covers the requested scopes." |
| `Always` | "Always show the consent screen regardless of stored grants." |

A dynamically registered client is, by construction, **not first-party** — it
registered itself through a protocol endpoint rather than being entered by an
administrator. So `None`, whose own documentation calls it the first-party
default, is wrong for it by definition. `Always` is safe but asks the user again
on every authorization after they have already decided, which trains people to
click through the screen that exists to make them stop and read. `FirstTime` asks
once and honours the answer. `crates/sui-id/src/http/handlers/dynamic_register.rs:198`
already sets it.

**So the code is right and the record is missing.** That asymmetry is the risk: a
future reader finds a security-relevant default with no stated rationale, and the
cheapest resolution for them is to "simplify" it to the enum's `#[default]`.
Writing it down is what prevents that, and it is the same failure mode as the
`create` bug this came from — a value that mattered, held in only one place.

**Note for contrast:** admin-created clients get `ConsentPolicy::default()`, i.e.
`None`, at `crates/sui-id-core/src/identity/admin/clients.rs:91`. That *is* the
first-party case, so it is consistent — but it too is unstated.

**If you agree, I would write a short RFC** recording both defaults and their
reasoning. I am not proposing to change behaviour.

## Q2 — should enabling a dynamic client surface its consent policy?

**Recommendation: yes — but I found something larger while measuring it, and it
changes what I would ask for.**

**The administrator cannot tell a self-registered client from one they created.**
`registered_via` appears **zero times** in `crates/sui-id-web/src/pages/clients.rs`.
It is persisted, it is audited, and it is never shown. The column exists precisely
to distinguish the two, and the only place a human could act on it does not
display it.

**And enabling is one click.** `crates/sui-id-web/src/pages/clients.rs:49-51` is a
bare `POST` to `/admin/clients/{id}/disabled` with a hidden `disabled=false` — no
intermediate page, no confirmation, no context:

```
<form method="post" action=disabled_url class="inline-el">
    <input type="hidden" name="disabled" value=action_target />
```

So the sequence that produced the bug we just fixed is also the sequence the UI
makes easiest: a client appears in the list, an administrator sees a name and an
"Enable" button, clicks it, and has now admitted a party that registered itself,
with no indication of either fact. RFC 008 P4's disabled-on-creation control is
doing real work here — it is the only thing standing between dynamic registration
and a live client — and the UI presents dismissing that control as a single
unlabelled click.

**What I would ask for, in priority order:**

1. **Show `registered_via` in the client list and detail.** Cheapest, and it fixes
   the general case rather than one path. Without it, nothing else here is
   legible.
2. **Make enabling a `dynamic` client a confirmation step** that states what the
   client asked for — its redirect URIs, its scopes, and its consent policy — and
   requires an explicit action. Not a browser dialog; a page. Enabling an
   admin-created client can stay one click, because the administrator created it.
3. **Leave the consent policy editable where it already is.** The edit form has a
   `consent_policy` `<select>` (`clients.rs:352-360`), so nothing new is needed to
   *change* it — only to *see* it at the moment it matters.

**This is not M2a, and I am not folding it in.** M2a is the transactional audit
seam; this is admin UI and a registration-trust question. If you want it, it
should be its own RFC, and I would put it after M2a rather than interleaved —
with the caveat that item 1 is small enough that holding it for a cycle is its own
kind of debt.

**No incident, and none was possible.** There are no deployments, and dynamic
registration requires an administrator-issued token. This is a design gap, not an
exposure.

## What I need from you

| | |
|---|---|
| Q1 | Agree `FirstTime` stays, and may I write the RFC recording both defaults? |
| Q2 | Do you want items 1–3, a subset, or none — and as its own RFC after M2a? |
