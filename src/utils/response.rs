use crate::db::request::save_response_body;
use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use futures_util::StreamExt;
use log::error;
use std::sync::Arc;
use tokio::sync::Mutex;

use std::pin::Pin;
use std::task::{Context, Poll};

pub struct StreamAdapter<
    P: futures_core::Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin,
> {
    parent: P,
}

impl<P: futures_core::Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin>
    StreamAdapter<P>
{
    pub fn new(parent: P) -> StreamAdapter<P> {
        StreamAdapter { parent }
    }
}

impl<P: futures_core::Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin>
    futures_core::Stream for StreamAdapter<P>
{
    type Item = Result<Bytes, anyhow::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.parent.poll_next_unpin(cx).map_err(anyhow::Error::from)
    }
}

pub type ModelStream =
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

pub struct ResponseLogger {
    pub id: String,
    pub buffer: Arc<Mutex<Vec<u8>>>,
}

impl Drop for ResponseLogger {
    fn drop(&mut self) {
        let id = self.id.clone();
        let buffer = self.buffer.clone();
        tokio::spawn(async move {
            let bytes = {
                let lock = buffer.lock().await;
                lock.clone()
            };
            if let Ok(text) = String::from_utf8(bytes) {
                if let Err(e) = save_response_body(&id, &text).await {
                    error!("Failed to save response body for request {id}: {e}");
                }
            } else {
                error!("Failed to parse response body as UTF-8 for request {id}");
            }
        });
    }
}

pub fn wrap_response_with_logging(
    mut response: ModelResponse,
    request_id: String,
) -> ModelResponse {
    response.request_id = Some(request_id.clone());
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let logger = ResponseLogger {
        id: request_id,
        buffer: buffer.clone(),
    };

    let original_body = std::mem::replace(
        &mut response.body,
        Box::new(Box::pin(futures_util::stream::empty())),
    );

    let wrapped_body = original_body.inspect(move |chunk| {
        if let Ok(bytes) = chunk {
            let buffer = logger.buffer.clone();
            let bytes = bytes.clone();
            tokio::spawn(async move {
                let mut lock = buffer.lock().await;
                lock.extend_from_slice(&bytes);
            });
        }
    });

    response.body = Box::new(Box::pin(wrapped_body));
    response
}
