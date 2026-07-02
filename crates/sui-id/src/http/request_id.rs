//! Request-id propagation.
//!
//! Each incoming request gets a `X-Request-Id`. If the caller
//! supplied one we keep it (capped to a sane length), otherwise we
//! generate a fresh UUIDv4. The id is:
//!
//! - Set as a header on the response so a caller / reverse proxy can
//!   correlate.
//! - Attached to a `tracing` span that wraps handler execution, so
//!   every log line emitted from inside the handler — both
//!   `tracing::info!` calls and our own `events::emit` — carries the
//!   id automatically.
//! - Stashed in a request extension under [`RequestId`] for handlers
//!   that want to read it directly (e.g. to put it in an
//!   [`events::Context`]).
//!
//! No correlation across reverse-proxy boundaries beyond this — we
//! deliberately don't accept opaque trace headers like
//! `traceparent` from outside the trust boundary because that's an
//! injection vector for noise into the operator's logs. Operators
//! who want full distributed tracing run an OpenTelemetry collector
//! with its own auth.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;
use uuid::Uuid;

pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Cap on accepted inbound request-id length. Anything longer is
/// replaced with a fresh UUID — protects log lines from being padded
/// with junk by a malicious caller.
const MAX_LEN: usize = 64;

/// Extension type so handlers can extract the id with `Extension(RequestId)`.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

pub async fn middleware(mut req: Request, next: Next) -> Response {
    let id = req
        .headers()
        .get(&REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty() && s.len() <= MAX_LEN && s.chars().all(is_safe_id_char))
        .map(|s| s.to_owned())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    req.extensions_mut().insert(RequestId(id.clone()));

    let method = req.method().clone();
    let uri = req.uri().clone();

    // Wrap handler execution in a span carrying the id so log lines
    // emitted from inside have it automatically.
    let span = tracing::info_span!(
        "request",
        request_id = %id,
        method = %method,
        path = %uri.path(),
    );
    let _enter = span.enter();

    tracing::info!("request received");
    let started = Instant::now();
    drop(_enter);

    let mut resp = {
        let _enter = span.enter();
        next.run(req).await
    };

    let _enter = span.enter();
    let status = resp.status().as_u16();
    let latency_ms = started.elapsed().as_millis() as u64;
    if status >= 500 {
        tracing::warn!(status, latency_ms, "request completed with server error");
    } else {
        tracing::info!(status, latency_ms, "request completed");
    }

    if let Ok(value) = HeaderValue::from_str(&id) {
        resp.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    resp
}

fn is_safe_id_char(c: char) -> bool {
    // ASCII alphanumeric, '-', '_', or '.'. Generous enough for
    // upstream trace ids; tight enough that we don't end up with
    // newlines or control bytes in our logs.
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'
}
