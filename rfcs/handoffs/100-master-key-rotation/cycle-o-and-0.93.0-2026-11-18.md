# Cycle O and `0.93.0` — the order, after what `0.92.0` changed

**Proposed.** 2026-11-18, by the architect, after `0.92.0` was published
(`ec1de8c`).
> ## Decided 2026-11-19, on §3's two questions
> 1. **The order is approved**: RFC 100 stage 1 (landed `92b3d01`), then RFC 148,
>    then stages 2–4.
> 2. **The account-creation run is later in cycle O**, not now. The architect
>    raises it again before cycle O's preparation; RFC 097's draft waits for it.
**Whose turn.** The owner's, for §3. The architect's, for the first
dispatch, which does not wait for it (§2).
**What already stands.** Cycle O is **authorized** and was re-scoped
2026-11-16 (ROADMAP, *Release cycles*). Its row:
- RFC 100 stages 1–4: the startup key check (its upgrade rule confirmed
  2026-11-16), the preconditions and `--check`, the journal, recovery;
- RFC 148, with `clap`, which closes RFC 107;
- RFC 097 (M5) drafted alongside, *"once stage 14 has closed M4-B"*.

This document changes none of that scope. It sets an order, and names the
one thing `0.92.0` left that the row did not foresee.

## 1 — what `0.92.0` changed: M4-B is still open

Stage 14 ran 2026-11-18 on `c30056b` against Duende's public demo
([record](../096-upstream-oidc-federation/096-b1-stage14-live-run-record-2026-11-18.md)).
The security path, the refusal, disable and delete all passed live. **Account
creation did not**: the demo's client sends the email from userinfo, not in
the ID token, which is the only source RFC 096 accepts. It was decided
2026-11-18 that **M4-B stays open until account creation is proven live**,
early in this cycle. **RFC 097 cannot start until then**, so that run gates
M5.

**The run needs an upstream that puts `email` and `email_verified` in the ID
token.** Google does by default with the `email` scope. It needs about 15
minutes of the owner's setup:
- a Google Cloud project;
- an OAuth consent screen in *Testing*, with the owner's own account as its
  one test user;
- a *Web application* OAuth client with the redirect URI
  `http://localhost:8801/auth/federated/callback`.

The architect then runs the same runbook's steps 7–10 against it. **The
record holds no personal data:** the test account's Google email and the client
secret are redacted, the rule the runbook kept for exactly this case.

## 2 — the order of the builds

| # | Work | Why here |
|---|---|---|
| **O1** | **Landed 2026-11-19** (`92b3d01`). **RFC 100 stage 1**: the sentinel and the startup key check, with the confirmed upgrade rule | The long pole goes first, as in cycle N. It touches startup and `resolve`, not the command parser |
| **O2** | **RFC 148**: one command registry, `clap`; closes RFC 107. **O2a landed 2026-11-19** — [`../148-one-command-system/0.93.0-o2a-one-registry-2026-11-19.md`](../148-one-command-system/0.93.0-o2a-one-registry-2026-11-19.md); **O2b dispatched 2026-11-19** — [`../148-one-command-system/0.93.0-o2b-init-and-a-legible-map-2026-11-19.md`](../148-one-command-system/0.93.0-o2b-init-and-a-legible-map-2026-11-19.md) (`init`, the first run, and O2a's three carried items) | Before stage 2, so `admin rotate-key --check` is written once, on the new parser, not twice |
| **O3** | **RFC 100 stage 2**: preconditions, and `--check` | On `clap` |
| **O4** | **RFC 100 stage 3**: the journal | |
| **O5** | **RFC 100 stage 4**: startup recovery | Recovery lands before stage 5's write path, as the stage rules require |
| **S** | **The account-creation run** (§1) | **Whenever the owner can**, independent of O1–O5. It closes M4-B, and RFC 097's draft starts the same day |
| **P** | the preparation | |

**O1 is dispatched now.** It depends on nothing in §3.

## 3 — decisions to put

1. **This order** (§2): RFC 100 stage 1, then RFC 148, then stages 2–4.
   Recommended.
2. **The account-creation run against Google** (§1), and when: the owner's
   15 minutes of setup, at a time that suits. Recommended early, because it
   gates RFC 097.
