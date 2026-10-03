// RFC 134 D1: no ambient cookie jar on the federation egress client.
// `reqwest`'s `cookie_store` builder method exists only when the `cookies`
// feature is enabled -- a transitive Cargo feature this crate does not
// request and (by feature unification) no other crate in the workspace
// may request either, or this fixture starts compiling and the test that
// runs it fails. There is no `cfg(feature = "cookies")` this crate could
// check instead: Cargo features are visible via `cfg` only inside the
// crate that declares them, not to a dependent looking at a transitive
// dependency's feature flags from outside.

fn attempt() {
    let _ = reqwest::Client::builder().cookie_store(true);
}

fn main() {}
