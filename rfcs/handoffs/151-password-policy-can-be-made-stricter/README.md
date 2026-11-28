# RFC 151 — handoff

**RFC.** [`../../done/151-password-policy-can-be-made-stricter.md`](../../done/151-password-policy-can-be-made-stricter.md)
**Status.** **Accepted 2026-11-16** ([design review record](design-review-2026-11-16.md)). **Dispatched
2026-11-25** as `0.94.0` P5: [`0.94.0-p5-the-minimum-can-be-raised-2026-11-25.md`](0.94.0-p5-the-minimum-can-be-raised-2026-11-25.md). It is Track D of the operator-experience track, built in cycle P
as a freeze exception decided 2026-11-15. **Closed 2026-11-28**: Implemented (v0.95.0), moved to `done/` on the [`0.95.0` closure assessment](../149-the-console-follows-the-operator/closure-assessment-0.95.0-2026-11-28.md), once Q2 put the field under *Sign-in*.

## What exists

**Built in `0.94.0` P5** (`a3e8dfc`, 2026-11-25): migration 0055, S11, the *Authentication* tab's field, and the localized hints and refusals. It ships in `0.94.0`. **It stays Accepted until RFC 149 lands**: its closure prerequisite says the minimum is raised *"from the *Sign-in* settings"*, and that tab is RFC 149's rename of today's *Authentication* tab (cycle Q). Every other prerequisite is met.
