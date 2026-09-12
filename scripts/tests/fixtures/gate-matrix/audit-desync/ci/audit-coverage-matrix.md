# Audit coverage matrix — A3.2 fixture (desynchronised)

Three deliberate desyncs, so a self-test that catches only some of them cannot
be mistaken for proof of a bidirectional check with a complete namespace view:

  * the **user** row below with no source literal — the forward check must
    fail on it;
  * the **client** literal in crates/ with no row here — the backward check
    must fail on it;
  * the **webauthn** literal in crates/ with no row here — the backward check
    must fail on it, and can only do so because the namespace allowlist is
    derived from the code (G13-b); the hand-written list before it could not
    see this namespace at all.

The auth.login pair is in sync, so the extraction is demonstrably working
rather than returning nothing; the user fixture_declared pair is in sync so
that the user namespace is declared in code.

Note for editors: the script extracts *every* backtick-quoted event name in
this file, prose included, so the desynced names are deliberately written
without backticks outside the table. Writing them normally here would put a
missing name back in the matrix and quietly weaken this fixture.

| Event | Emitted by |
|---|---|
| `auth.login` | `crates/fixture-audit/src/lib.rs` |
| `user.fixture_declared` | `crates/fixture-audit/src/lib.rs` |
| `user.in_matrix_only` | nothing — this row is the forward-direction desync |
