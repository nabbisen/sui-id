# RFC 152 — handoff

**RFC.** [`../../accepted/152-what-a-relying-party-can-rely-on.md`](../../accepted/152-what-a-relying-party-can-rely-on.md)
**Status.** **Accepted 2026-11-26** ([design review record](design-review-2026-11-26.md)). Not yet dispatched. It is built
in cycle Q beside RFC 149, as authorized 2026-11-25: stages Q6 (D1–D3), Q7 (D5) and Q8 (D4, D6–D8) of [the plan](../149-the-console-follows-the-operator/cycle-q-and-0.95.0-2026-11-26.md), whose §1 proposes answers to its three open questions. **Amended 2026-11-26** with D8: what a relying party must do for sui-id's guarantees to reach its users.

## Documents

| Document | Contents |
|---|---|
| [`design-review-2026-11-26.md`](design-review-2026-11-26.md) | The acceptance record, and the three questions it left open |
| [`2026-11-25-introspection-active-after-disable.md`](2026-11-25-introspection-active-after-disable.md) | A finding of the same measurement, held under SECURITY.md until fixed |
| [`0.94.0-p8-introspection-agrees-with-userinfo-2026-11-25.md`](0.94.0-p8-introspection-agrees-with-userinfo-2026-11-25.md) | Its fix, `0.94.0` P8, landed `31106bf` |

## What exists

The RFC. Its findings were measured at `648e2ae`, answering a relying party's questions. The OIDC reference already says
what is true today: `prompt` and `max_age` are ignored and no `auth_time` is issued, and `email_verified` is always `false`.
