// RFC 094 §"Class-A transaction seam", the finding that blocked the
// user-administration wave (2026-09-08): U01 ("admin create user") is the
// first *real* command whose actor is `ActorRequirement::Required` and
// whose `system_principal: forbidden;` declaration is load-bearing, not
// provisional. This is the "negative proof, demonstrated not asserted"
// the finding's item 4 asked for — the same property already proven
// against `ProofOnlyForbiddenSystemPrincipalCommand`
// (`system_principal_forbidden_cannot_use_system_actor.rs`), but against
// a command with a real inventory row and a real caller, not a stub that
// exists only to make this kind of fixture possible.
//
// `U01` does not implement `SystemPrincipalPermitted`, and
// `for_system_actor` — bounded `impl<C: SystemPrincipalPermitted>
// AuthorizedCommandContext<C>` — has no applicable method for it. An
// admin-attributed command therefore cannot construct a context that
// omits the actor: not by convention, by a compile error naming the
// missing bound.

use sui_id_store::commands::U01;
use sui_id_store::registry::AuthorizedCommandContext;

fn attempt() {
    let _ = AuthorizedCommandContext::<U01>::for_system_actor(None);
}

fn main() {}
