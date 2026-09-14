//! A3.2 fixture: a test file in the fixture crate.
//!
//! Never compiled. It holds the only literal of an event the matrix
//! registers, so it stands in for a test assertion that mentions an event
//! name. Since G13-d the literal scan skips test files, so this literal is
//! not evidence of a writer and the forward check must report the row.

#[test]
fn audit_page_echoes_the_filter() {
    let body = String::new();
    assert!(body.contains("auth.written_only_by_tests"));
}
