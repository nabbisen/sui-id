# Verification Artifacts

This directory holds formal and semi-formal verification artifacts that are
useful for design review but are not part of the release package.

- `refresh_rotation.tla` models refresh-token rotation safety properties.
- `pilot-reports.md` records the RFC 086 pilot results and recommendations.

Keep these files in sync with the corresponding Rust protocol code when the
protocol changes.
