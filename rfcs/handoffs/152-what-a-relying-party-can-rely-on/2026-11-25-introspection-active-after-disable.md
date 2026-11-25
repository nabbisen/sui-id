# Held — introspection answers `active: true` for a disabled or deleted user's access token

> ## Published 2026-11-26, with the fix (`31106bf`)
> Held in `.git-exclude/staging/` under `.github/SECURITY.md` until the fix
> shipped to `main`, as O7's finding was. The text below is as it was held,
> except one sentence reworded so that it attributes no ruling not yet made.

**Found.** 2026-11-25, by the architect, while measuring planeter's question 2 at `648e2ae`.
**Held, not tracked.** `.github/SECURITY.md`'s scope includes *"unauthorised acceptance of a token issued by sui-id"*. Until it is ruled a vulnerability or a design choice, it stays out of git, out of commit messages, and out of
both letters' drafts.

## What was measured

- `POST /oauth2/introspect` with an access token: `try_introspect_access`
  (`crates/sui-id-core/src/oidc/oauth_token.rs`) checks the signature, `exp`, the audience against the authenticating client,
  and the revocation deny-list. Then `active_from_access` returns `active: true`. **It never reads the user's `is_disabled` or
  `is_deleted`.** Its comment says this is deliberate for a deleted user: *"we still report the token active until exp"*.
- **Disabling (U02) and deleting a user revoke sessions, refresh tokens and auth codes in one transaction.** Neither adds the
  user's outstanding access tokens to the deny-list.
- **`/oauth2/userinfo` does refuse** a disabled or deleted user (`handlers/oidc.rs`, `if row.is_disabled || row.is_deleted`).
  So two endpoints answer the same question differently.

## The consequence

A resource server that relies on introspection keeps accepting a disabled or deleted user for up to the access-token
lifetime: `tokens.access_lifetime_secs`, 900 seconds by default, but operator-configurable upward. An administrator who
disables a compromised account reasonably believes it has stopped working everywhere sui-id is asked.

## What a fix would look like (for a dispatch, if ruled a defect)

- `active_from_access` reads the user and answers `active: false` when it is disabled or deleted, matching userinfo.
- A test: issue a token, disable the user, and introspection is inactive. Mutation: the user check removed.
- A CHANGELOG *Security* entry on release, written as the disclosure.

## For the owner

Is this a vulnerability, to be fixed and disclosed under SECURITY.md, or an accepted window, to be documented? The architect
recommends **fixing it**. The window is bounded, but the inconsistency with userinfo is exactly the kind of thing an operator
cannot be expected to know.
