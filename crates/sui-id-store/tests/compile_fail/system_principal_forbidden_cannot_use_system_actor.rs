// RFC 094 §"Class-A transaction seam": `AuthorizedCommandContext<C>` is
// created "only by consuming a successful authorization decision for
// command type `C`, or by a sealed CLI/system authority adapter for
// commands whose descriptor permits that principal." Listed among the
// RFC's own compile-negative fixtures: "invoke a system-principal adapter
// for a user-only command".
//
// `ProofOnlyForbiddenSystemPrincipalCommand` is declared
// `system_principal: forbidden;`, so it does not implement
// `SystemPrincipalPermitted`, and `for_system_actor` — bounded
// `impl<C: SystemPrincipalPermitted> AuthorizedCommandContext<C>` — has no
// applicable method for it. This must fail to compile, not merely panic
// or return `Err`, per the checklist's own requirement for a negative
// proof of this property.

use sui_id_store::registry::AuthorizedCommandContext;
use sui_id_store::registry::ProofOnlyForbiddenSystemPrincipalCommand;

fn attempt() {
    let _ = AuthorizedCommandContext::<ProofOnlyForbiddenSystemPrincipalCommand>::for_system_actor(
        None,
    );
}

fn main() {}
