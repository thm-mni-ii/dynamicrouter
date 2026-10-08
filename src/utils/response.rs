use crate::db::request::save_response_body_and_tokens;
use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use futures_util::StreamExt;
use log::error;
use serde_json::Value;
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
    pub rate_limit_headers: Option<Vec<(String, String)>>,
    pub body: ModelStream,
}

impl IntoResponse for ModelResponse {
    fn into_response(self) -> Response {
        let mut builder = Response::builder().status(self.status);
        let is_event_stream = self
            .content_type
            .as_deref()
            .is_some_and(|ct| ct.contains("text/event-stream"));

        if let Some(ct) = self.content_type {
            builder = builder.header(header::CONTENT_TYPE, ct);
        }
        if let Some(cl) = self.content_length {
            builder = builder.header(header::CONTENT_LENGTH, cl);
        }
        if is_event_stream {
            builder = builder.header(header::CACHE_CONTROL, "no-cache, no-transform");
            builder = builder.header("x-accel-buffering", "no");
        }
        if let Some(rid) = self.request_id {
            builder = builder.header("x-request-id", rid);
        }
        if let Some(headers) = self.rate_limit_headers {
            for (name, val) in headers {
                if let (Ok(h_name), Ok(h_val)) = (
                    axum::http::HeaderName::from_bytes(name.as_bytes()),
                    axum::http::HeaderValue::from_str(&val),
                ) {
                    builder = builder.header(h_name, h_val);
                }
            }
        }

        builder.body(Body::from_stream(self.body)).unwrap()
    }
}

pub struct RequestGuard {
    request_id: String,
    completed: bool,
}

impl RequestGuard {
    pub fn new(request_id: String) -> Self {
        Self {
            request_id,
            completed: false,
        }
    }

    pub fn mark_completed(&mut self) {
        self.completed = true;
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        if !self.completed {
            let id = self.request_id.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::db::request::request_aborted(&id).await {
                    error!("Failed to mark aborted request {id}: {e}");
                }
            });
        }
    }
}

pub struct ResponseLogger {
    pub id: String,
    pub prompt_tokens: i64,
    pub buffer: Arc<Mutex<Vec<u8>>>,
}

pub fn parse_tokens_from_response(text: &str, default_prompt_tokens: i64) -> (i64, i64, i64) {
    // 1. Check if the response is valid JSON (non-streaming standard OpenAI response)
    if let Ok(json_val) = serde_json::from_str::<Value>(text)
        && let Some(usage) = json_val.get("usage")
    {
        let p = usage.get("prompt_tokens").and_then(Value::as_i64).unwrap_or(default_prompt_tokens);
        let c = usage.get("completion_tokens").and_then(Value::as_i64).unwrap_or(0);
        let t = usage.get("total_tokens").and_then(Value::as_i64).unwrap_or(p + c);
        return (p, c, t);
    }

    // 2. Check if the response is Server-Sent Events stream (SSE chunks)
    let mut sse_prompt_tokens = None;
    let mut sse_completion_tokens = None;
    let mut sse_total_tokens = None;
    let mut generated_text = String::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(data_str) = trimmed.strip_prefix("data: ") {
            if data_str == "[DONE]" {
                continue;
            }
            if let Ok(chunk_json) = serde_json::from_str::<Value>(data_str) {
                // If usage block is embedded in stream chunk
                if let Some(usage) = chunk_json.get("usage") {
                    if let Some(p) = usage.get("prompt_tokens").and_then(Value::as_i64) {
                        sse_prompt_tokens = Some(p);
                    }
                    if let Some(c) = usage.get("completion_tokens").and_then(Value::as_i64) {
                        sse_completion_tokens = Some(c);
                    }
                    if let Some(t) = usage.get("total_tokens").and_then(Value::as_i64) {
                        sse_total_tokens = Some(t);
                    }
                }
                // Accumulate generated text for estimation if usage is absent
                if let Some(choices) = chunk_json.get("choices").and_then(Value::as_array) {
                    for choice in choices {
                        if let Some(delta) = choice.get("delta")
                            && let Some(content) = delta.get("content").and_then(Value::as_str)
                        {
                            generated_text.push_str(content);
                        }
                    }
                }
            }
        }
    }

    if let (Some(p), Some(c), Some(t)) = (sse_prompt_tokens, sse_completion_tokens, sse_total_tokens) {
        return (p, c, t);
    }

    // 3. Fallback: estimate from text
    let prompt_tokens = default_prompt_tokens.max(1);
    let completion_tokens = if generated_text.is_empty() {
        // Estimate from full body if not SSE
        ((text.chars().count() as f64) / 4.0).ceil() as i64
    } else {
        ((generated_text.chars().count() as f64) / 3.8).ceil() as i64
    };

    (prompt_tokens, completion_tokens, prompt_tokens + completion_tokens)
}

impl Drop for ResponseLogger {
    fn drop(&mut self) {
        let id = self.id.clone();
        let prompt_tokens = self.prompt_tokens;
        let buffer = self.buffer.clone();
        tokio::spawn(async move {
            let bytes = {
                let lock = buffer.lock().await;
                lock.clone()
            };
            if let Ok(text) = String::from_utf8(bytes) {
                let (p, c, t) = parse_tokens_from_response(&text, prompt_tokens);
                if let Err(e) = save_response_body_and_tokens(&id, &text, p, c, t).await {
                    error!("Failed to save response body & tokens for request {id}: {e}");
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
    prompt_tokens: i64,
) -> ModelResponse {
    response.request_id = Some(request_id.clone());
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let logger = ResponseLogger {
        id: request_id,
        prompt_tokens,
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
