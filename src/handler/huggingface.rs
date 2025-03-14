use crate::config::CONFIG;
use crate::handler::utils::{ByteSize, ModelRef, QuantType};
use once_cell::sync::Lazy;
use reqwest::Client;
use secrecy::ExposeSecret;
use serde_json::Value;
use std::fmt::Debug;
use std::future::Future;

static CLIENT: Lazy<Client> = Lazy::new(Client::new);

#[derive(Debug, Clone)]
pub struct HfModel {
    pub parameter_count: u64,
    pub storage_size: u64,
}

pub async fn get_model_information(id: String) -> Result<HfModel, anyhow::Error> {
    let mut builder = CLIENT.get(format!("https://huggingface.co/api/models/{id}"));
    if let Some(token) = &CONFIG.huggingface.token {
        builder = builder.bearer_auth(token.expose_secret());
    }

    let res: Value = builder.send().await?.error_for_status()?.json().await?;
    let pc = res
        .get("safetensors")
        .and_then(|p| p.get("total"))
        .and_then(serde_json::Value::as_i64)
        .and_then(|t| t.try_into().ok())
        .ok_or(Error::GetParameter())?;
    let size = res
        .get("usedStorage")
        .and_then(serde_json::Value::as_i64)
        .and_then(|t| t.try_into().ok())
        .ok_or(Error::GetStorageSize())?;

    Ok(HfModel {
        parameter_count: pc,
        storage_size: size,
    })
}

pub fn calculate_needed_space(parameters: u64, quant: QuantType) -> ByteSize {
    let fb = parameters as f64 * quant.get_byte_size();
    ByteSize::from_bytes(fb.ceil() as u64)
}

pub async fn get_from_config_or_calculate(
    model: &ModelRef,
    get_model_information: impl Future<Output = Result<HfModel, anyhow::Error>>,
) -> Result<ByteSize, anyhow::Error> {
    Ok(calculate_needed_space(
        match CONFIG
            .models
            .get(&model.to_string())
            .and_then(|model| model.param_count)
        {
            None => get_model_information.await?.parameter_count,
            Some(param_count) => param_count,
        },
        model.quant.unwrap_or(QuantType::FP16),
    ))
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("missing total safetensors parameter count")]
    GetParameter(),
    #[error("missing storage size")]
    GetStorageSize(),
}
