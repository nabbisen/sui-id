# RFC 152 — handoff

**RFC.** [`../../accepted/152-what-a-relying-party-can-rely-on.md`](../../accepted/152-what-a-relying-party-can-rely-on.md)
**Status.** **Accepted 2026-11-26** ([design review record](design-review-2026-11-26.md)). Not yet dispatched. Its stages are
Q6 (D1–D3, and D9), **Q6b** (federated freshness, D2), Q7 (D5) and Q8 (D4, D6–D8) of [the plan](../149-the-console-follows-the-operator/cycle-q-and-0.95.0-2026-11-26.md).
**They ship in `0.96.0`**, since `0.95.0` was cut after RFC 149 on 2026-11-28. **Amended 2026-11-28**: its open questions
are resolved, and D9 is added ([considered](open-questions-considered-2026-11-28.md)). **Amended 2026-11-26** with D8: what a relying party must do for sui-id's guarantees to reach its users.

## Documents

| Document | Contents |
|---|---|
| [`design-review-2026-11-26.md`](design-review-2026-11-26.md) | The acceptance record, and the three questions it left open |
| [`open-questions-considered-2026-11-28.md`](open-questions-considered-2026-11-28.md) | **The three questions, and a fourth, re-measured against the philosophy.** One proposal is corrected: federated sign-in would have let `auth_time` overstate a sign-in. Decided 2026-11-28, each as recommended |
| [`2026-11-25-introspection-active-after-disable.md`](2026-11-25-introspection-active-after-disable.md) | A finding of the same measurement, held under SECURITY.md until fixed |
| [`0.94.0-p8-introspection-agrees-with-userinfo-2026-11-25.md`](0.94.0-p8-introspection-agrees-with-userinfo-2026-11-25.md) | Its fix, `0.94.0` P8, landed `31106bf` |

## What exists

The RFC. Its findings were measured at `648e2ae`, answering a relying party's questions. The OIDC reference already says
what is true today: `prompt` and `max_age` are ignored and no `auth_time` is issued, and `email_verified` is always `false`.
