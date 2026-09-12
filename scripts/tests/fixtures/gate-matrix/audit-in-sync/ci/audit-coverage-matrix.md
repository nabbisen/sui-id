# Audit coverage matrix — A3.2 fixture (in sync)

The clean counterpart to `audit-desync`. Every backticked event name here has
a matching string literal in `crates/`, and every literal there has a row, so
G13 must pass. Names are extracted by the grep in
`scripts/check-audit-matrix.sh`: a backticked event name whose namespace the
script derives from the fixture crate's `action:` literals.

| Event | Emitted by |
|---|---|
| `auth.login` | `crates/fixture-audit/src/lib.rs` |
| `user.create` | `crates/fixture-audit/src/lib.rs` |
