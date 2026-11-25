# RFC 152 — handoff

**RFC.** [`../../proposed/152-what-a-relying-party-can-rely-on.md`](../../proposed/152-what-a-relying-party-can-rely-on.md)
**Status.** **Proposed 2026-11-25.** Not dispatched. Building waits for its acceptance. The schedule proposed for it, cycle
Q beside RFC 149, is authorized.

## Documents

| Document | Contents |
|---|---|
| [`2026-11-25-introspection-active-after-disable.md`](2026-11-25-introspection-active-after-disable.md) | A finding of the same measurement, held under SECURITY.md until fixed |
| [`0.94.0-p8-introspection-agrees-with-userinfo-2026-11-25.md`](0.94.0-p8-introspection-agrees-with-userinfo-2026-11-25.md) | Its fix, `0.94.0` P8, landed `31106bf` |

## What exists

The RFC. Its findings were measured at `648e2ae`, answering a relying party's questions. The OIDC reference already says
what is true today: `prompt` and `max_age` are ignored and no `auth_time` is issued, and `email_verified` is always `false`.
