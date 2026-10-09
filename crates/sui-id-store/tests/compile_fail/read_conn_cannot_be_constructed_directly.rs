// RFC 096-B1 stage 0: `ReadConn`'s only field is private and its
// constructor (`ReadConn::new`) is `pub(crate)` — nothing outside
// `sui-id-store` can wrap a `&Connection` it already holds into a
// `ReadConn` on its own; the only way to obtain one is
// `Database::with_read` / `with_read_sync`. Same proof shape as this
// crate's `session_insert_is_private.rs`; same reason it matters as the
// sealed-type fixtures in `sui-id`: a type that can be constructed outside
// its own invariants guarantees nothing.

use sui_id_store::ReadConn;

fn attempt(conn: &rusqlite::Connection) -> ReadConn<'_> {
    ReadConn { conn }
}

fn main() {}
