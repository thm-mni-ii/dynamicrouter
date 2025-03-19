use crate::config::CONFIG;
use crate::handler::utils::{ByteSize, ParameterCount};
use once_cell::sync::Lazy;
use reqwest::Client;
use secrecy::ExposeSecret;
use serde_json::Value;
use std::fmt::Debug;

static CLIENT: Lazy<Client> = Lazy::new(Client::new);

#[derive(Debug, Copy, Clone)]
pub struct HfModel {
    pub parameter_count: Option<ParameterCount>,
    pub storage_size: Option<ByteSize>,
}

impl HfModel {
    pub async fn get(id: String) -> Result<HfModel, anyhow::Error> {
        let mut builder = CLIENT.get(format!("https://huggingface.co/api/models/{id}"));
        if let Some(token) = &CONFIG.huggingface.token {
            builder = builder.bearer_auth(token.expose_secret());
        }

        let res: Value = builder.send().await?.error_for_status()?.json().await?;
        let pc = res
            .get("safetensors")
            .and_then(|p| p.get("total"))
            .and_then(serde_json::Value::as_i64)
            .and_then(|t| t.try_into().ok());
        let size = res
            .get("usedStorage")
            .and_then(serde_json::Value::as_i64)
            .and_then(|t| t.try_into().ok());

        Ok(HfModel {
            parameter_count: pc,
            storage_size: size,
        })
    }
}
