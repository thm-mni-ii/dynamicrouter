use crate::config::CONFIG;
use crate::handlers::GenerationType;
use crate::handlers::spawner::factories::models::ModelError::{NonRetryableError, RetryableError};
use crate::utils::response::{ModelResponse, StreamAdapter};
use async_trait::async_trait;
use log::warn;
use once_cell::sync::Lazy;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;

#[allow(dead_code)]
#[async_trait::async_trait]
pub trait Model: Send + Sync {
    async fn id(&self) -> &str;
    async fn name(&self) -> &str;
    async fn generate(
        &self,
        generation_type: GenerationType,
        request: &Value,
    ) -> Result<ModelResponse, ModelError<anyhow::Error>>;
    async fn invoke(
        &self,
        path: &str,
        body: &Value,
    ) -> Option<Result<ModelResponse, ModelError<anyhow::Error>>>;
    async fn serialize(&self) -> Result<Vec<u8>, anyhow::Error>;
}

#[derive(Debug)]
pub enum ModelError<E> {
    RetryableError(E),
    NonRetryableError(E),
}

impl<E> ModelError<E> {
    pub fn is_retryable(&self) -> bool {
        matches!(*self, RetryableError(_))
    }

    pub fn is_non_retryable(&self) -> bool {
        !self.is_retryable()
    }
}

impl<E: Display> Display for ModelError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RetryableError(e) | NonRetryableError(e) => e.fmt(f),
        }
    }
}

impl From<ModelError<anyhow::Error>> for anyhow::Error {
    fn from(value: ModelError<anyhow::Error>) -> Self {
        match value {
            RetryableError(e) | NonRetryableError(e) => e,
        }
    }
}

impl<E: std::error::Error + Send + Sync + 'static> From<E> for ModelError<anyhow::Error> {
    fn from(value: E) -> Self {
        RetryableError(value.into())
    }
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

    async fn request(
        &self,
        path: &str,
        request: &Value,
    ) -> Result<ModelResponse, ModelError<anyhow::Error>> {
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
            return Err(ModelError::RetryableError(
                HttpModelError::UnexpectedStatus(status).into(),
            ));
        }

        Ok(ModelResponse {
            status,
            content_type: res
                .headers()
                .get("Content-Type")
                .map(|ct| ct.to_str().unwrap().to_string()),
            content_length: res.content_length(),
            request_id: None,
            rate_limit_headers: None,
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

    async fn generate(
        &self,
        generation_type: GenerationType,
        request: &Value,
    ) -> Result<ModelResponse, ModelError<anyhow::Error>> {
        self.request(
            match generation_type {
                GenerationType::Completions => "/v1/chat/completions",
                GenerationType::Embeddings => "/v1/embeddings",
                GenerationType::Responses => "/v1/responses",
            },
            request,
        )
        .await
    }

    async fn invoke(
        &self,
        path: &str,
        body: &Value,
    ) -> Option<Result<ModelResponse, ModelError<anyhow::Error>>> {
        Some(self.request(path, body).await)
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

    async fn generate(
        &self,
        generation_type: GenerationType,
        request: &Value,
    ) -> Result<ModelResponse, ModelError<anyhow::Error>> {
        let mut i = 0;
        while i < CONFIG.model_timeout / CONFIG.model_retry_interval {
            match self.parent.generate(generation_type, request).await {
                Ok(res) => {
                    return Ok(res);
                }
                Err(err) => {
                    if err.is_non_retryable() {
                        return Err(err);
                    }
                    warn!("Retrying after error: {err}");
                    sleep(Duration::from_secs(CONFIG.model_retry_interval.into())).await;
                }
            }
            i += 1;
        }
        Err(RetryingModelError::RetriesExceeded().into())
    }

    async fn invoke(
        &self,
        path: &str,
        body: &Value,
    ) -> Option<Result<ModelResponse, ModelError<anyhow::Error>>> {
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
            }
            i += 1;
        }
        Some(Err(RetryingModelError::RetriesExceeded().into()))
    }

    async fn serialize(&self) -> Result<Vec<u8>, anyhow::Error> {
        self.parent.serialize().await
    }
}

#[derive(Debug, Error)]
enum RetryingModelError {
    #[error("model not responding after exceeding retries")]
    RetriesExceeded(),
}

#[derive(Debug, Error)]
enum HttpModelError {
    #[error("unexpected status {0}")]
    UnexpectedStatus(StatusCode),
}
