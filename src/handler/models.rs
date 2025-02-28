use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use async_trait::async_trait;
use axum::body::Body;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use reqwest::{Client, StatusCode, Url};
use serde_json::Value;
use futures_util::StreamExt;
use log::warn;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

type ModelStream = Box<dyn futures_core::Stream<Item = Result<Bytes, anyhow::Error>> + Send + Unpin>;

pub struct ModelResponse {
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub body: ModelStream
}

impl IntoResponse for ModelResponse {
    fn into_response(self) -> Response {
        let mut builder = Response::builder()
            .status(StatusCode::OK);
        builder = match self.content_type {
            Some(ct) => builder.header(header::CONTENT_TYPE, ct),
            None => builder
        };
        builder = match self.content_length {
            Some(cl) => builder.header(header::CONTENT_LENGTH, cl),
            None => builder,
        };

        builder.body(Body::from_stream(self.body)).unwrap()
    }
}

struct StreamAdapter<P: futures_core::Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin> {
    parent: P
}

impl <P: futures_core::Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin> StreamAdapter<P> {
    pub fn new(parent: P) -> StreamAdapter<P> {
        StreamAdapter {parent}
    }
}

impl <P: futures_core::Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin> futures_core::Stream for StreamAdapter<P> {
    type Item = Result<Bytes, anyhow::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.parent.poll_next_unpin(cx).map_err(anyhow::Error::from)
    }
}

#[async_trait::async_trait]
pub trait Model: Send + Sync {
    async fn id(&self) -> &str;
    async fn name(&self) -> &str;
    async fn generate(&self, request: &Value) -> Result<ModelResponse, anyhow::Error>;
    async fn serialize(&self) -> Result<Vec<u8>, anyhow::Error>;
}

pub type DynModel = Arc<dyn Model>;

#[derive(Serialize, Deserialize)]
pub struct HttpModel {
    id: String,
    name: String,
    url: Url,
    token: Option<String>,
    #[serde(skip)]
    client: Client,
}

impl HttpModel {
    pub fn new_dyn(id: String, name: String, url: Url, token: Option<String>) -> DynModel {
        Arc::new(Self { id, name, url, token, client: Client::default() })
    }

    pub fn deserialize(data: &[u8]) -> Result<DynModel, anyhow::Error> {
        Ok(Arc::new(serde_json::from_slice::<Self>(data)?))
    }
}

#[async_trait::async_trait]
impl Model for HttpModel {
    async fn id(&self) -> &str {
        &self.id
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn generate(&self, request: &Value) -> Result<ModelResponse, anyhow::Error> {
        let mut builder = self.client.post(self.url.clone()).json(request);
        builder = match &self.token {
            Some(token) => builder.bearer_auth(token),
            None => builder,
        };
        let res = builder.send().await?.error_for_status()?;

        Ok(ModelResponse {
            content_type: res.headers().get("Content-Type").map(|ct| ct.to_str().unwrap().to_string()),
            content_length: res.content_length(),
            body: Box::new(StreamAdapter::new(res.bytes_stream())),
        })
    }

    async fn serialize(&self) -> Result<Vec<u8>, anyhow::Error> {
        Ok(serde_json::to_vec(self)?)
    }
}

pub struct RetryingModel {
    parent: DynModel,
}

impl RetryingModel {
    pub fn new_dyn(parent: DynModel) -> DynModel {
        Arc::new(RetryingModel { parent })
    }
}

#[async_trait]
impl Model for RetryingModel {
    async fn id(&self) -> &str {
        self.parent.id().await
    }

    async fn name(&self) -> &str {
        self.parent.name().await
    }

    async fn generate(&self, request: &Value) -> Result<ModelResponse, anyhow::Error> {
        loop {
            match self.parent.generate(request).await {
                Ok(res) => {
                    return Ok(res);
                },
                Err(err) => {
                    warn!("Retrying after error: {err}");
                    sleep(Duration::from_secs(60)).await;
                }
            };
        }
    }

    async fn serialize(&self) -> Result<Vec<u8>, anyhow::Error> {
        self.parent.serialize().await
    }
}
