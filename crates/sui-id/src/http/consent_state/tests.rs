use super::*;

fn req() -> ConsentRequest {
    ConsentRequest {
        client_id: "c".into(),
        redirect_uri: "https://rp.test/cb".into(),
        scope: "openid profile".into(),
        state: Some("s".into()),
        nonce: None,
        code_challenge: "ch".into(),
        code_challenge_method: "S256".into(),
    }
}

#[test]
fn a_sealed_value_opens_for_its_session_until_it_expires() {
    let key = MasterKey::generate();
    let s = SessionId::new();
    let v = seal(&key, s, 1_000, &req());
    assert_eq!(open(&key, s, 1_000, &v), Some(req()));
    assert_eq!(open(&key, s, 1_000 + LIFETIME_SECS, &v), Some(req()));
    assert_eq!(open(&key, s, 1_001 + LIFETIME_SECS, &v), None);
}

#[test]
fn a_value_opens_for_no_other_session() {
    let key = MasterKey::generate();
    let v = seal(&key, SessionId::new(), 1_000, &req());
    assert_eq!(open(&key, SessionId::new(), 1_000, &v), None);
}

#[test]
fn a_value_opens_under_no_other_key() {
    let s = SessionId::new();
    let v = seal(&MasterKey::generate(), s, 1_000, &req());
    assert_eq!(open(&MasterKey::generate(), s, 1_000, &v), None);
}

#[test]
fn a_changed_payload_or_tag_does_not_open() {
    let key = MasterKey::generate();
    let s = SessionId::new();
    let v = seal(&key, s, 1_000, &req());
    let (body, tag) = v.split_once('.').unwrap();

    // A payload that says something else, under the original tag.
    let mut other = req();
    other.scope = "openid profile email".into();
    let changed_body = Base64UrlUnpadded::encode_string(
        &serde_json::to_vec(&Payload {
            exp: 1_000 + LIFETIME_SECS,
            req: other,
        })
        .unwrap(),
    );
    assert_eq!(open(&key, s, 1_000, &format!("{changed_body}.{tag}")), None);

    // The original payload under a different tag.
    let mut bad_tag = tag.to_owned();
    bad_tag.replace_range(..1, if tag.starts_with('A') { "B" } else { "A" });
    assert_eq!(open(&key, s, 1_000, &format!("{body}.{bad_tag}")), None);
}

#[test]
fn a_value_that_is_not_a_sealed_value_does_not_open() {
    let key = MasterKey::generate();
    let s = SessionId::new();
    for junk in [
        "",
        ".",
        "a.b",
        "{\"user_id\":\"x\"}",
        "not-base64!.not-base64!",
    ] {
        assert_eq!(open(&key, s, 0, junk), None, "{junk:?}");
    }
}

#[test]
fn the_cookie_is_scoped_hidden_from_script_and_secure_when_configured() {
    for secure in [false, true] {
        let c = cookie("v".into(), secure).to_string();
        assert!(c.starts_with("sui_id_consent=v"), "{c}");
        assert!(c.contains("Path=/oauth2/consent"), "{c}");
        assert!(c.contains("HttpOnly"), "{c}");
        assert!(c.contains("SameSite=Lax"), "{c}");
        assert!(c.contains("Max-Age=300"), "{c}");
        assert_eq!(c.contains("Secure"), secure, "{c}");
    }
}

#[test]
fn a_value_carries_no_session_id() {
    let key = MasterKey::generate();
    let s = SessionId::new();
    let v = seal(&key, s, 1_000, &req());
    let (body, _) = v.split_once('.').unwrap();
    let json = String::from_utf8(Base64UrlUnpadded::decode_vec(body).unwrap()).unwrap();
    assert!(!json.contains(&s.to_string()), "{json}");
    assert!(!json.contains("user"), "{json}");
}
