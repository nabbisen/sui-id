# Audit coverage matrix — A3.2 fixture (desynchronised)

Two deliberate desyncs, one in each direction, so a self-test that catches
only one cannot be mistaken for proof of a bidirectional check:

  * the **user** row below has no string literal in crates/ — the forward
    check must fail on it;
  * the **client** literal in crates/ has no row here — the backward check
    must fail on it.

The auth.login pair is in sync, so the extraction is demonstrably working
rather than returning nothing.

Note for editors: the script extracts *every* backtick-quoted event name in
this file, prose included, so the two desynced names are deliberately written
without backticks outside the table. Writing them normally here would put the
missing name back in the matrix and quietly halve this fixture.

| Event | Emitted by |
|---|---|
| `auth.login` | `crates/fixture-audit/src/lib.rs` |
| `user.in_matrix_only` | nothing — this row is the forward-direction desync |
