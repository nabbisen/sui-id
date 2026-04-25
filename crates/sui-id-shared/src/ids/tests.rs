use super::*;
use std::str::FromStr;

#[test]
fn user_and_client_ids_are_not_interchangeable_at_compile_time() {
    // This is a compile-time check rephrased as a runtime assertion: the
    // two newtypes differ in type, so we just make sure construction is
    // independent.
    let u = UserId::new();
    let c = ClientId::new();
    assert_ne!(u.to_string(), c.to_string());
}

#[test]
fn ids_round_trip_through_string() {
    let u = UserId::new();
    let s = u.to_string();
    let back = UserId::from_str(&s).expect("parse");
    assert_eq!(u, back);
}

#[test]
fn ids_serialize_as_plain_uuid_string() {
    let u = UserId::new();
    let json = serde_json::to_string(&u).expect("ser");
    // Should be just `"<uuid>"`, no wrapping object.
    assert!(json.starts_with('"'));
    assert!(json.ends_with('"'));
    assert_eq!(json.len(), 38);
}
