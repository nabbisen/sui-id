# RFC 137 — handoff

**RFC.** [`../../done/137-tests-live-beside-not-inside.md`](../../done/137-tests-live-beside-not-inside.md)

**Status: Implemented, closed 2026-10-07** — the RFC header carries the
approval and the Level B evidence. Scoped to `crates/*/src/`. **No open work.**

Five stages, all landed. The counts below are the measured ones; the figures in
the original dispatches (77, then 17/24/26) came from a grep that counted files
already in the compliant form.

| Stage | Scope | Landed |
|---|---|---|
| [`g21-and-stage1-2026-10-06.md`](g21-and-stage1-2026-10-06.md) | G21, then 10 files across the three small crates | `134e93b`, self-tests `f418540` |
| [`stage2-sui-id-2026-10-06.md`](stage2-sui-id-2026-10-06.md) | `sui-id`, 13 files | `7af2174` |
| [`stage3-sui-id-core-2026-10-06.md`](stage3-sui-id-core-2026-10-06.md) | `sui-id-core`, 18 files / 22 modules | `d69b400` |
| [`stage4-sui-id-store-2026-10-06.md`](stage4-sui-id-store-2026-10-06.md) | `sui-id-store`, 18 files / 19 modules | `6bfc45b` |
| [`stage5-id-token-2026-10-07.md`](stage5-id-token-2026-10-07.md) | `id_token.rs`, the deferral withdrawn | `add4797` |
| [`closure-review-2026-10-07.md`](closure-review-2026-10-07.md) | closure record | `f61c2cd` |
