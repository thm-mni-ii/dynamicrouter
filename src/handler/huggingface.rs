use crate::config::CONFIG;
use crate::handler::utils::QuantType;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde_json::Value;
use std::fmt::Debug;
use log::debug;

static CLIENT: Lazy<Client> = Lazy::new(Client::new);

pub async fn get_total_parameter_count(id: &str) -> Result<u64, anyhow::Error> {
    let mut builder = CLIENT.get(format!("https://huggingface.co/api/models/{id}"));
    if let Some(token) = &CONFIG.huggingface_token {
        builder = builder.bearer_auth(token);
    }

    let res: Value = builder.send().await?.error_for_status()?.json().await?;
    debug!("{res:?}");
    let pc = res.get("safetensors").and_then(|p| p.get("total")).and_then(serde_json::Value::as_i64).and_then(|t| t.try_into().ok()).ok_or(Error::GetParameter())?;
    Ok(pc)
}

pub fn calculate_needed_space(parameters: u64, quant: QuantType) -> u64 {
    let fb = parameters as f64 * quant.get_byte_size();
    fb.ceil() as u64
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("missing total safetensors parameter count")]
    GetParameter(),
}