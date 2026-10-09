// RFC 096-B1 stage 0 / RFC 094's 2026-08-26 amendment: statement-level
// `sqlite3_stmt_readonly` cannot see a side-effecting application function
// — that surface must stay off the dependency graph entirely, which is a
// Cargo feature this crate does not request. There is no `cfg(feature =
// "functions")` this crate could check instead (same reason as the
// `cookies` fixture in `sui-id`'s own `tests/compile_fail/`): Cargo
// features are visible via `cfg` only inside the crate that declares them.
// `rusqlite::functions` is itself `#[cfg(feature = "functions")]`, so
// naming it is the only way to probe the real, resolved feature set — if
// this feature is ever turned on, by this crate or by feature unification
// with any other crate in the workspace that wants it, this fixture starts
// compiling and the test that runs it fails.

fn attempt() -> rusqlite::functions::FunctionFlags {
    rusqlite::functions::FunctionFlags::SQLITE_UTF8
}

fn main() {}
