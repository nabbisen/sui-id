// RFC 102 B6: "The raw step-up touch has no production caller." Step-up
// freshness is recorded only by L05 (`auth.step_up.success`, committed with the
// factor's consumption) and by L02 (which sets the method for a new session).
//
// `sessions::record_step_up_within_tx` and `sessions::set_step_up_method_within_tx`
// are `pub(crate)` in `sui-id-store`, so code outside the crate — `sui-id-core`,
// the HTTP layer, or anything else — cannot mark a session fresh, or set the
// method that made it fresh, without going through one of those commands. This
// fixture names both from outside the crate.

use chrono::Utc;
use sui_id_shared::ids::SessionId;
use sui_id_store::repos::sessions;

fn record(conn: &rusqlite::Connection, id: SessionId) {
    let _ = sessions::record_step_up_within_tx(conn, id, "totp", Utc::now());
}

fn set_method(conn: &rusqlite::Connection, id: SessionId) {
    let _ = sessions::set_step_up_method_within_tx(conn, id, "totp");
}

fn main() {}
