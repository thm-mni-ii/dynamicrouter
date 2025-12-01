use axum::Router;
use axum::http::HeaderName;
use axum::routing::get;
use reqwest::header;

type StaticFile = ([(HeaderName, &'static str); 1], &'static [u8]);

async fn serve_index() -> StaticFile {
    (
        [(header::CONTENT_TYPE, "text/html")],
        include_bytes!("../ui/dist/index.html"),
    )
}

async fn serve_css() -> StaticFile {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_bytes!("../ui/dist/pico.lime.min.css"),
    )
}

async fn serve_js() -> StaticFile {
    (
        [(header::CONTENT_TYPE, "text/javascript")],
        include_bytes!("../ui/dist/ui.js"),
    )
}

async fn serve_wasm() -> StaticFile {
    (
        [(header::CONTENT_TYPE, "application/wasm")],
        include_bytes!("../ui/dist/ui_bg.wasm"),
    )
}

pub fn new_static_router() -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/pico.lime.min.css", get(serve_css))
        .route("/ui.js", get(serve_js))
        .route("/ui_bg.wasm", get(serve_wasm))
}
