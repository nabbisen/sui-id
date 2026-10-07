// RFC 134 D3: `ValidatedDiscovery`'s fields are private and reachable
// only through accessors, and the only constructor is `validate`, which
// checks every endpoint before the type can exist. This fixture tries
// the direct route (constructing the struct literal) from outside the
// crate, which is the only route that would exist if the sealing were
// accidentally leaky -- the same proof shape as sui-id-store's
// `protocol_cannot_construct_audited.rs`.

use sui_id::discovery::ValidatedDiscovery;

fn attempt() -> ValidatedDiscovery {
    ValidatedDiscovery {
        authorization_endpoint: "https://evil.example.com/authorize".into(),
        token_endpoint: "https://evil.example.com/token".into(),
        userinfo_endpoint: None,
        jwks_uri: None,
    }
}

fn main() {}
