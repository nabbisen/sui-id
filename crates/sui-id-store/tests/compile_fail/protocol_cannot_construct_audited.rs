// RFC 094 §"Mechanically complete durable-write universe": "WriteTx<Protocol>,
// <Operational>, and <Bootstrap> have distinct runners and cannot construct
// Audited<T>."
//
// `Database::protocol` never hands the closure anything that leads to
// `Audited<T>` — there is no method on `WriteTx<'_, Protocol>` that produces
// one, and `Audited`'s own fields are private with no public constructor.
// This fixture tries the direct route (constructing the struct literal) from
// outside the crate, which is the only route that would exist if the sealing
// were accidentally leaky.

use sui_id_store::registry::{Audited, WriteTx};

fn attempt<P>(_write: &mut WriteTx<'_, P>) -> Audited<()> {
    Audited {
        value: (),
        receipt: sui_id_store::registry::AuditReceipt { _private: () },
    }
}

fn main() {}
