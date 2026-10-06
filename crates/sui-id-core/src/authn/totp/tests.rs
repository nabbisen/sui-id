use super::*;

/// RFC 6238 test vectors, Appendix B (HMAC-SHA1, ASCII secret
/// "12345678901234567890" — 20 bytes — in dec).
const RFC_SECRET: &[u8] = b"12345678901234567890";

#[tokio::test]
async fn rfc6238_appendix_b_vectors() {
    // From the RFC: time, expected code (HMAC-SHA1).
    // step = time / 30
    let cases = [
        (59i64, 94287082u32),
        (1111111109, 7081804),
        (1111111111, 14050471),
        (1234567890, 89005924),
        (2000000000, 69279037),
        (20000000000, 65353130),
    ];
    for (t, expected) in cases {
        let step = t / 30;
        let got = code_for_step(RFC_SECRET, step).await;
        // The RFC publishes 8-digit values; sui-id uses 6 digits, so
        // truncate the published expected to its last 6 digits.
        let want_6 = expected % 1_000_000;
        assert_eq!(got, want_6, "step {step}");
    }
}

#[tokio::test]
async fn verify_accepts_current_step() {
    let now = 1_700_000_000_i64;
    let step = now.div_euclid(STEP_SECS);
    let code = code_for_step(RFC_SECRET, step).await;
    let got = verify(RFC_SECRET, now, code, 0).await;
    assert_eq!(got, Some(step));
}

#[tokio::test]
async fn verify_accepts_minus_one_step() {
    let now = 1_700_000_000_i64;
    let step = now.div_euclid(STEP_SECS) - 1;
    let code = code_for_step(RFC_SECRET, step).await;
    assert_eq!(verify(RFC_SECRET, now, code, 0).await, Some(step));
}

#[tokio::test]
async fn verify_rejects_replay_within_window() {
    let now = 1_700_000_000_i64;
    let step = now.div_euclid(STEP_SECS);
    let code = code_for_step(RFC_SECRET, step).await;
    // First time: accepted.
    assert_eq!(verify(RFC_SECRET, now, code, 0).await, Some(step));
    // Second time, recording the previous step: rejected.
    assert!(verify(RFC_SECRET, now, code, step).await.is_none());
}

#[tokio::test]
async fn verify_rejects_wrong_code() {
    let now = 1_700_000_000_i64;
    assert!(verify(RFC_SECRET, now, 000000, 0).await.is_none());
}

#[tokio::test]
async fn verify_rejects_overlong_code() {
    // 7-digit submission — must fail without trying the HMAC.
    assert!(
        verify(RFC_SECRET, 1_700_000_000, 1_234_567, 0)
            .await
            .is_none()
    );
}

#[tokio::test]
async fn base32_round_trip_known_vectors() {
    assert_eq!(base32_encode(b"").await, "");
    assert_eq!(base32_encode(b"f").await, "MY");
    assert_eq!(base32_encode(b"fo").await, "MZXQ");
    assert_eq!(base32_encode(b"foo").await, "MZXW6");
    assert_eq!(base32_encode(b"foob").await, "MZXW6YQ");
    assert_eq!(base32_encode(b"fooba").await, "MZXW6YTB");
    assert_eq!(base32_encode(b"foobar").await, "MZXW6YTBOI");
}

#[tokio::test]
async fn otpauth_uri_has_required_fields() {
    let uri = otpauth_uri("sui-id", "alice", b"01234567890123456789").await;
    assert!(uri.starts_with("otpauth://totp/sui-id:alice?"));
    assert!(uri.contains("secret="));
    assert!(uri.contains("issuer=sui-id"));
    assert!(uri.contains("algorithm=SHA1"));
    assert!(uri.contains("digits=6"));
    assert!(uri.contains("period=30"));
}
