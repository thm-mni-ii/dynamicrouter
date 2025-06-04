use crate::config::CONFIG;
use crate::utils::response::ModelResponse;
use anyhow::Error;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::StreamExt;
use log::warn;
use once_cell::sync::Lazy;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;

struct StreamAdapter<P: futures_core::Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin> {
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

#[async_trait::async_trait]
pub trait Model: Send + Sync {
    async fn id(&self) -> &str;
    async fn name(&self) -> &str;
    async fn generate(&self, request: &Value) -> Result<ModelResponse, anyhow::Error>;
    async fn serialize(&self) -> Result<Vec<u8>, anyhow::Error>;
    async fn invoke(
        &self,
        path: &str,
        body: &Value,
    ) -> Option<Result<ModelResponse, anyhow::Error>>;
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

static OK_STATUS_CODES: Lazy<HashSet<StatusCode>> = Lazy::new(|| {
    HashSet::from_iter(vec![
        StatusCode::OK,
        StatusCode::BAD_REQUEST,
        StatusCode::UNPROCESSABLE_ENTITY,
        StatusCode::INTERNAL_SERVER_ERROR,
    ])
});

impl HttpModel {
    pub fn new_dyn(id: String, name: String, url: Url, token: Option<String>) -> DynModel {
        Arc::new(Self {
            id,
            name,
            url,
            token,
            client: Client::default(),
        })
    }

    pub fn deserialize(data: &[u8]) -> Result<DynModel, anyhow::Error> {
        Ok(Arc::new(serde_json::from_slice::<Self>(data)?))
    }

    async fn request(&self, path: &str, request: &Value) -> Result<ModelResponse, anyhow::Error> {
        let mut url = self.url.clone();
        url.set_path(path);
        let mut builder = self.client.post(url).json(request);
        builder = match &self.token {
            Some(token) => builder.bearer_auth(token),
            None => builder,
        };
        let res = builder.send().await?;
        let status = res.status();
        if !OK_STATUS_CODES.contains(&status) {
            return Err(ModelError::UnexpectedStatus(status).into());
        }

        Ok(ModelResponse {
            status,
            content_type: res
                .headers()
                .get("Content-Type")
                .map(|ct| ct.to_str().unwrap().to_string()),
            content_length: res.content_length(),
            body: Box::new(StreamAdapter::new(res.bytes_stream())),
        })
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
        self.request("/v1/chat/completions", request).await
    }

    async fn serialize(&self) -> Result<Vec<u8>, anyhow::Error> {
        Ok(serde_json::to_vec(self)?)
    }

    async fn invoke(
        &self,
        path: &str,
        body: &Value,
    ) -> Option<Result<ModelResponse, anyhow::Error>> {
        Some(self.request(path, body).await)
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
        let mut i = 0;
        while i < CONFIG.model_timeout / CONFIG.model_retry_interval {
            match self.parent.generate(request).await {
                Ok(res) => {
                    return Ok(res);
                }
                Err(err) => {
                    warn!("Retrying after error: {err}");
                    sleep(Duration::from_secs(CONFIG.model_retry_interval.into())).await;
                }
            };
            i += 1;
        }
        Err(ModelError::RetriesExceeded().into())
    }

    async fn serialize(&self) -> Result<Vec<u8>, anyhow::Error> {
        self.parent.serialize().await
    }

    async fn invoke(&self, path: &str, body: &Value) -> Option<Result<ModelResponse, Error>> {
        let mut i = 0;
        while i < CONFIG.model_timeout / CONFIG.model_retry_interval {
            match self.parent.invoke(path, body).await {
                Some(Ok(res)) => {
                    return Some(Ok(res));
                }
                Some(Err(err)) => {
                    warn!("Retrying after error: {err}");
                    sleep(Duration::from_secs(CONFIG.model_retry_interval.into())).await;
                }
                None => return None,
            };
            i += 1;
        }
        Some(Err(ModelError::RetriesExceeded().into()))
    }
}

#[derive(Debug, Error)]
enum ModelError {
    #[error("model not responding after exceeding retries")]
    RetriesExceeded(),
    #[error("unexpected status {0}")]
    UnexpectedStatus(StatusCode),
}
