// RFC 096-B1 stage 0: `ReadConn` offers `prepare`, and the statement it
// returns offers `query_row` / `query_map` — nothing execute-shaped. This
// attempts the obvious escape on a legitimately-typed `&ReadConn`, proving
// the write surface is simply absent from the type, not merely refused at
// runtime by the interrogation (`read_conn.rs`'s own tests cover that).

use sui_id_store::ReadConn;

fn attempt(read: &ReadConn<'_>) {
    let mut stmt = read.prepare("SELECT 1").unwrap();
    stmt.execute([]).unwrap();
}

fn main() {}
