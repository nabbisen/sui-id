# Verification Artifacts

This directory holds formal and semi-formal verification artifacts that are
useful for design review but are not part of the release package.

- `refresh_rotation.tla` models refresh-token rotation safety properties.
- `pilot-reports.md` records the RFC 086 pilot results and recommendations.
- `verus-scoping-note-2026-11-14.md` proposes one first Verus target, TOTP
  step selection, with a proof that was run, and what adopting it costs.
  Design only: nothing it describes is built.

Keep these files in sync with the corresponding Rust protocol code when the
protocol changes.
