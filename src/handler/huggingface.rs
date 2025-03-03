use crate::config::CONFIG;
use crate::handler::utils::QuantType;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde_json::Value;
use std::fmt::Debug;

static CLIENT: Lazy<Client> = Lazy::new(Client::new);

#[derive(Debug, Clone)]
pub struct HfModel {
    pub parameter_count: u64,
    pub storage_size: u64,
}

pub async fn get_model_information(id: &str) -> Result<HfModel, anyhow::Error> {
    let mut builder = CLIENT.get(format!("https://huggingface.co/api/models/{id}"));
    if let Some(token) = &CONFIG.hf.token {
        builder = builder.bearer_auth(token);
    }

    let res: Value = builder.send().await?.error_for_status()?.json().await?;
    let pc = res.get("safetensors").and_then(|p| p.get("total")).and_then(serde_json::Value::as_i64).and_then(|t| t.try_into().ok()).ok_or(Error::GetParameter())?;
    let size = res.get("usedStorage").and_then(serde_json::Value::as_i64).and_then(|t| t.try_into().ok()).ok_or(Error::GetStorageSize())?;

    Ok(HfModel {parameter_count: pc, storage_size: size})
}

pub fn calculate_needed_space(parameters: u64, quant: QuantType) -> u64 {
    let fb = parameters as f64 * quant.get_byte_size();
    fb.ceil() as u64
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("missing total safetensors parameter count")]
    GetParameter(),
    #[error("missing storage size")]
    GetStorageSize(),
}