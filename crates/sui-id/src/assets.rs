//! Embedded static assets.
//!
//! sui-id ships its UI inline (the stylesheet is part of the rendered HTML),
//! so the static asset surface is intentionally tiny: a favicon and a
//! robots.txt. They live in `/static` at the workspace root.

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use include_dir::{include_dir, Dir};

static STATIC_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/static");

pub async fn serve(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    let trimmed = path.trim_start_matches('/');
    if let Some(file) = STATIC_DIR.get_file(trimmed) {
        let mime = mime_for(trimmed);
        return ([(header::CONTENT_TYPE, mime)], file.contents()).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

fn mime_for(path: &str) -> &'static str {
    if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".txt") {
        "text/plain; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}
