// RFC 102 A4: "The raw session insert has no production caller. Sessions for
// sign-in are created only through this RFC's commands."
//
// `sessions::insert` and `sessions::insert_within_tx` are `pub(crate)` in
// `sui-id-store`, so code outside the crate — `sui-id-core`, the HTTP layer,
// or anything else — cannot create a session without going through one of
// the sign-in commands L01-L04, each of which commits the session with its
// audit event. This fixture names both from outside the crate.

use sui_id_store::models::SessionRow;
use sui_id_store::repos::sessions;

async fn attempt(db: &sui_id_store::Database, row: &SessionRow) {
    let _ = sessions::insert(db, row).await;
}

fn attempt_within_tx(conn: &rusqlite::Connection, row: &SessionRow) {
    let _ = sessions::insert_within_tx(conn, row);
}

fn main() {}
