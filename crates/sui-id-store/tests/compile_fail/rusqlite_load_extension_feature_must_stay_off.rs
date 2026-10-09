// RFC 096-B1 stage 0 / RFC 094's 2026-08-26 amendment: loading an
// arbitrary shared library's SQLite extension is the most direct
// side-effecting surface statement-level `readonly()` cannot see at all —
// same reasoning as the `functions`/`vtab` fixtures beside this one.
// `Connection::load_extension_disable` is `#[cfg(feature =
// "load_extension")]` and safe to call (no `unsafe` needed), so it is the
// plainest probe of this one feature.

fn attempt(conn: &rusqlite::Connection) {
    conn.load_extension_disable().unwrap();
}

fn main() {}
