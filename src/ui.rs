use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::RustEmbed;

pub const CSP_VALUE: &str = "default-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; img-src 'self' data: blob:; connect-src 'self'; media-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none';";

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Asset;

pub async fn static_handler(req: Request) -> impl IntoResponse {
    let path = req.uri().path().trim_start_matches('/');

    // If path is empty, serve index.html
    let lookup_path = if path.is_empty() { "index.html" } else { path };

    // Try finding the exact static asset first
    if let Some(file) = Asset::get(lookup_path) {
        let mime = mime_guess::from_path(lookup_path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap_or(HeaderValue::from_static("application/octet-stream")))
            .header(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP_VALUE))
            .header(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"))
            .header(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"))
            .header(header::REFERRER_POLICY, HeaderValue::from_static("strict-origin-when-cross-origin"))
            .body(Body::from(file.data))
            .unwrap();
    }

    // SPA fallback: for all non-asset routes (e.g. /chat, /tokens, /admin/models), serve index.html
    if let Some(index) = Asset::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))
            .header(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP_VALUE))
            .header(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"))
            .header(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"))
            .header(header::REFERRER_POLICY, HeaderValue::from_static("strict-origin-when-cross-origin"))
            .body(Body::from(index.data))
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP_VALUE))
        .body(Body::from("404 Not Found"))
        .unwrap()
}

pub fn new_static_router() -> Router {
    Router::new().fallback(get(static_handler))
}
