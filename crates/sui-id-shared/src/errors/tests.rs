use super::*;

#[test]
fn http_status_mapping_is_stable() {
    assert_eq!(ApiErrorCode::Unauthorized.http_status(), 401);
    assert_eq!(ApiErrorCode::Forbidden.http_status(), 403);
    assert_eq!(ApiErrorCode::BadRequest.http_status(), 400);
    assert_eq!(ApiErrorCode::NotFound.http_status(), 404);
    assert_eq!(ApiErrorCode::Conflict.http_status(), 409);
    assert_eq!(ApiErrorCode::InvalidState.http_status(), 409);
    assert_eq!(ApiErrorCode::TooManyRequests.http_status(), 429);
    assert_eq!(ApiErrorCode::Protocol.http_status(), 400);
    assert_eq!(ApiErrorCode::Internal.http_status(), 500);
}

#[test]
fn api_error_payload_does_not_leak_cause() {
    let err = ApiError::new(ApiErrorCode::Internal, "something went wrong", "req-123");
    let json = serde_json::to_string(&err).expect("serialize");
    // Only the explicitly listed fields should be in the payload.
    assert!(json.contains("internal"));
    assert!(json.contains("req-123"));
    assert!(!json.contains("source"));
    assert!(!json.contains("backtrace"));
}

#[test]
fn api_error_with_protocol_code_round_trips() {
    let err = ApiError::new(ApiErrorCode::Protocol, "bad grant", "req-9")
        .with_protocol_code("invalid_grant");
    let s = serde_json::to_string(&err).expect("ser");
    let back: ApiError = serde_json::from_str(&s).expect("de");
    assert_eq!(back.protocol_code.as_deref(), Some("invalid_grant"));
}
