use super::*;

struct StubBreached(u64);
#[async_trait::async_trait]
impl HibpClient for StubBreached {
    async fn check(&self, _password: &str) -> HibpCheckOutcome {
        HibpCheckOutcome::Breached { count: self.0 }
    }
}
struct StubClean;
#[async_trait::async_trait]
impl HibpClient for StubClean {
    async fn check(&self, _password: &str) -> HibpCheckOutcome {
        HibpCheckOutcome::NotBreached
    }
}
struct StubUnavailable;
#[async_trait::async_trait]
impl HibpClient for StubUnavailable {
    async fn check(&self, _password: &str) -> HibpCheckOutcome {
        HibpCheckOutcome::Unavailable
    }
}

#[tokio::test]
async fn parse_response_finds_match() {
    // Real-shape line for "P@ssw0rd" — SHA-1 of "P@ssw0rd"
    // is "21BD12DC183F740EE76F27B78EB39C8AD972A757", so the
    // first 5 chars are "21BD1" and the suffix is the rest.
    let body = "0001E1559DBC1641BCFD3A30E18AAB52CDA:1\r\n\
                    2DC183F740EE76F27B78EB39C8AD972A757:42\r\n\
                    FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF:99\r\n";
    let result = parse_response(body, "2DC183F740EE76F27B78EB39C8AD972A757");
    assert_eq!(result, HibpCheckOutcome::Breached { count: 42 });
}

#[tokio::test]
async fn parse_response_returns_not_breached_on_no_match() {
    let body = "0001E1559DBC1641BCFD3A30E18AAB52CDA:1\r\n\
                    FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF:99\r\n";
    let result = parse_response(body, "2DC183F740EE76F27B78EB39C8AD972A757");
    assert_eq!(result, HibpCheckOutcome::NotBreached);
}

#[tokio::test]
async fn parse_response_treats_zero_count_as_padding() {
    // Add-Padding lines have count 0; we treat a match with
    // count 0 as not-breached out of caution.
    let body = "2DC183F740EE76F27B78EB39C8AD972A757:0\r\n";
    let result = parse_response(body, "2DC183F740EE76F27B78EB39C8AD972A757");
    assert_eq!(result, HibpCheckOutcome::NotBreached);
}

#[tokio::test]
async fn parse_response_is_case_insensitive_on_suffix() {
    let body = "2dc183f740ee76f27b78eb39c8ad972a757:5\r\n";
    let result = parse_response(body, "2DC183F740EE76F27B78EB39C8AD972A757");
    assert_eq!(result, HibpCheckOutcome::Breached { count: 5 });
}

#[tokio::test]
async fn enforce_off_skips_check_entirely() {
    // Passing a "would-be-breached" stub but mode=Off must
    // not even consult the client.
    let stub = StubBreached(99);
    let result = enforce_hibp(HibpMode::Off, Some(&stub), "anything").await;
    assert_eq!(result, HibpEnforcement::Allowed);
}

#[tokio::test]
async fn enforce_warn_lets_breached_through_with_count() {
    let stub = StubBreached(42);
    let result = enforce_hibp(HibpMode::Warn, Some(&stub), "p").await;
    assert_eq!(result, HibpEnforcement::AllowedWithWarning { count: 42 });
}

#[tokio::test]
async fn enforce_block_refuses_breached() {
    let stub = StubBreached(42);
    let result = enforce_hibp(HibpMode::Block, Some(&stub), "p").await;
    assert_eq!(result, HibpEnforcement::Blocked { count: 42 });
}

#[tokio::test]
async fn enforce_warn_lets_clean_through() {
    let stub = StubClean;
    let result = enforce_hibp(HibpMode::Warn, Some(&stub), "p").await;
    assert_eq!(result, HibpEnforcement::Allowed);
}

#[tokio::test]
async fn enforce_block_lets_clean_through() {
    let stub = StubClean;
    let result = enforce_hibp(HibpMode::Block, Some(&stub), "p").await;
    assert_eq!(result, HibpEnforcement::Allowed);
}

#[tokio::test]
async fn enforce_warn_fail_open_when_unavailable() {
    let stub = StubUnavailable;
    let result = enforce_hibp(HibpMode::Warn, Some(&stub), "p").await;
    assert_eq!(result, HibpEnforcement::Allowed);
}

#[tokio::test]
async fn enforce_block_fail_open_when_unavailable() {
    // Crucial: block mode must NOT block when the API is
    // unreachable. This is the documented fail-open policy.
    let stub = StubUnavailable;
    let result = enforce_hibp(HibpMode::Block, Some(&stub), "p").await;
    assert_eq!(result, HibpEnforcement::Allowed);
}

#[tokio::test]
async fn enforce_hibp_or_reject_returns_bad_request_on_block() {
    let stub = StubBreached(7);
    let err = enforce_hibp_or_reject(HibpMode::Block, Some(&stub), "p")
        .await
        .expect_err("should reject");
    match err {
        CoreError::BadRequest(_) => {}
        other => panic!("expected BadRequest, got {:?}", other),
    }
}
