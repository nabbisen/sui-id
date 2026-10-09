// RFC 096-B1 stage 0 / RFC 094's 2026-08-26 amendment: statement-level
// `sqlite3_stmt_readonly` cannot see a side-effecting virtual table either
// — same reasoning as `rusqlite_functions_feature_must_stay_off.rs`, against
// `rusqlite`'s `vtab` feature instead of `functions`.

fn attempt(kind: rusqlite::vtab::VTabKind) -> rusqlite::vtab::VTabKind {
    kind
}

fn main() {}
