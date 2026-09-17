// RFC 102 stage 7: K01 (signing-key rotation) is the administrator's web
// rotation, so it is `system_principal: forbidden` with actor `Required`.
// `K01` does not implement `SystemPrincipalPermitted`, so no code can build a
// K01 context that omits the actor.

use sui_id_store::commands::K01;
use sui_id_store::registry::AuthorizedCommandContext;

fn attempt() {
    let _ = AuthorizedCommandContext::<K01>::for_system_actor(None);
}

fn main() {}
