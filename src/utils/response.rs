use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;

type ModelStream =
    Box<dyn futures_core::Stream<Item = Result<Bytes, anyhow::Error>> + Send + Unpin>;

pub struct ModelResponse {
    pub status: StatusCode,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub request_id: Option<String>,
    pub body: ModelStream,
}

impl IntoResponse for ModelResponse {
    fn into_response(self) -> Response {
        let mut builder = Response::builder().status(self.status);
        builder = match self.content_type {
            Some(ct) => builder.header(header::CONTENT_TYPE, ct),
            None => builder,
        };
        builder = match self.content_length {
            Some(cl) => builder.header(header::CONTENT_LENGTH, cl),
            None => builder,
        };
        builder = match self.request_id {
            Some(rid) => builder.header("x-request-id", rid),
            None => builder,
        };

        builder.body(Body::from_stream(self.body)).unwrap()
    }
}
