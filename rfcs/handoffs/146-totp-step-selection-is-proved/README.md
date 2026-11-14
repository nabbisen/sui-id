# RFC 146 — handoff

**RFC.** [`../../accepted/146-totp-step-selection-is-proved.md`](../../accepted/146-totp-step-selection-is-proved.md)
**Status.** **Accepted 2026-11-14** ([design review record](./design-review-2026-11-14.md)).
**Not dispatched**: the build waits for M7, the real-environment soak.

> **Also in this folder, and not RFC 146's build:** `0.91.0` S3, the Kani
> harnesses' removal, dispatched 2026-11-14 —
> [`0.91.0-s3-retire-the-kani-harnesses-2026-11-14.md`](./0.91.0-s3-retire-the-kani-harnesses-2026-11-14.md).

## Where it stands, 2026-11-14

The scoping note ([`../../../verification/verus-scoping-note-2026-11-14.md`](../../../verification/verus-scoping-note-2026-11-14.md))
recommended writing this RFC now, design only, and building it after the
soak, with no feature-freeze exception. That recommendation was authorized
the same day. So:

- **Before the build:** done. Reviewed and accepted 2026-11-14.
- **The build:** after M7, the real-environment soak, with no freeze
  exception.
- **No second target** until this one has run through a real change, and its
  cost has been measured (RFC §Non-goals, scoping note §6).

The proof itself already exists and has been run: the scoping note's §2
holds its source and the four results.
