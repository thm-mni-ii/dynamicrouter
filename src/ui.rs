use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::RustEmbed;

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
            .body(Body::from(file.data))
            .unwrap();
    }

    // SPA fallback: for all non-asset routes (e.g. /chat, /tokens, /admin/models), serve index.html
    if let Some(index) = Asset::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))
            .body(Body::from(index.data))
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("404 Not Found"))
        .unwrap()
}

pub fn new_static_router() -> Router {
    Router::new().fallback(get(static_handler))
}
