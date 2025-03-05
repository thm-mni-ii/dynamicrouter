use crate::config::{RunpodGpu, CONFIG};
use crate::handler::factories::ModelFactory;
use crate::handler::huggingface::{calculate_needed_space, get_model_information};
use crate::handler::models::{DynModel, HttpModel};
use crate::handler::utils::{build_docker_args, ModelRef, QuantType, GIB_IN_BYTE};
use log::{debug, info};
use once_cell::sync::Lazy;
use reqwest::{Client, Url};
use secrecy::ExposeSecret;
use serde_json::{json, Value};
use std::fmt::Debug;
use thiserror::Error;

pub struct RunpodModelFactory {
    client: Client,
}

impl RunpodModelFactory {
    pub fn new() -> RunpodModelFactory {
        RunpodModelFactory {
            client: Client::default(),
        }
    }
}

static RUNPOD_GRAPHQL: Lazy<Url> = Lazy::new(|| "https://api.runpod.io/graphql".parse().unwrap());
const RUNPOD_CREATE_MUTATION: &str = "mutation Mutation($input: PodFindAndDeployOnDemandInput) {  podFindAndDeployOnDemand(input: $input) {    id    imageName    env    machineId    machine {      podHostId      __typename    }    __typename  }}";
const RUNPOD_DESTROY_MUTATION: &str =
    "mutation terminatePod($input: PodTerminateInput!) {podTerminate(input: $input)}";

pub fn calculate_gpu_match(gpu: &RunpodGpu, needed: u64) -> Option<(String, u8, f64)> {
    let gpu_size = gpu.memory * GIB_IN_BYTE;
    let gpus_needed = (needed / gpu_size) + 1;
    if gpus_needed == 1 || gpus_needed == 2 || gpus_needed == 4 || gpus_needed == 8 {
        Some((
            gpu.id.clone(),
            gpus_needed as u8,
            gpu.price * gpus_needed as f64,
        ))
    } else {
        None
    }
}

fn calculate_needed_gpu(
    model: &ModelRef,
    needed_memory: u64,
) -> Result<(String, u8), anyhow::Error> {
    let mut options = CONFIG
        .factory
        .as_runpod()
        .gpus
        .iter()
        .filter_map(|gpu| calculate_gpu_match(gpu, needed_memory))
        .collect::<Vec<(String, u8, f64)>>();
    options.sort_by(|a, b| a.2.total_cmp(&b.2));

    debug!("Valid options for {model:?}: {options:?}");

    Ok(options
        .first()
        .map(|(model, count, _)| (model.clone(), *count))
        .ok_or(RunpodError::NoGpuConfig())?)
}

#[async_trait::async_trait]
impl ModelFactory for RunpodModelFactory {
    async fn create(&self, model: ModelRef) -> Result<DynModel, anyhow::Error> {
        let model_information = get_model_information(&model.get_hf_id()).await?;
        let needed_gpu_memory = calculate_needed_space(
            model_information.parameter_count,
            model.quant.unwrap_or(QuantType::FP16),
        );

        let (gpu_id, gpu_count) = calculate_needed_gpu(
            &model,
            needed_gpu_memory + CONFIG.factory.as_runpod().gpu_memory_margin * GIB_IN_BYTE,
        )?;
        info!("Using {gpu_count} x {gpu_id} for {model}");

        let mut input = json!({
            "cloudType":"SECURE",
            "containerDiskInGb": model_information.storage_size / GIB_IN_BYTE + CONFIG.factory.as_runpod().disk_storage_margin,
            "volumeInGb":0,
            "volumeMountPath": "/data",
            "gpuCount":gpu_count,
            "gpuTypeId":gpu_id,
            "templateId": CONFIG.factory.as_runpod().template_id,
            "imageName": CONFIG.factory.as_runpod().image,
            "ports":"80/http",
            "dockerArgs": build_docker_args(&model).join(" ")
        });
        if let Some(token) = &CONFIG.huggingface.token {
            input.as_object_mut().unwrap().insert(
                "env".to_string(),
                json!([{"key": "HF_TOKEN", "value": token.expose_secret()}]),
            );
        }

        let body = json!({"query": RUNPOD_CREATE_MUTATION, "operationName": "Mutation", "variables": {"input": input}});
        debug!("Sending {body:?} to runpod");

        let res: Value = self
            .client
            .post(RUNPOD_GRAPHQL.clone())
            .bearer_auth(CONFIG.factory.as_runpod().token.expose_secret())
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let id = res
            .get("data")
            .and_then(|c| {
                c.get("podFindAndDeployOnDemand")
                    .and_then(|c| c.get("id"))
                    .and_then(|id| id.as_str())
            })
            .ok_or(RunpodError::NoId())?;
        Ok(HttpModel::new_dyn(
            id.to_string(),
            model.to_string(),
            format!("https://{id}-80.proxy.runpod.net/v1/chat/completions").parse()?,
            None,
        ))
    }

    async fn destroy(&self, model: DynModel) -> Result<(), anyhow::Error> {
        let id = model.id().await;
        self.client.post(RUNPOD_GRAPHQL.clone())
            .bearer_auth(CONFIG.factory.as_runpod().token.expose_secret())
            .json(&json!({"query": RUNPOD_DESTROY_MUTATION, "operationName": "terminatePod", "variables": {"input": {"podId": id}}}))
            .send().await?.error_for_status()?;
        Ok(())
    }

    async fn dump(&self, model: &DynModel) -> Result<Vec<u8>, anyhow::Error> {
        model.serialize().await
    }

    async fn restore(&self, dump: &[u8]) -> Result<DynModel, anyhow::Error> {
        HttpModel::deserialize(dump)
    }
}

#[derive(Debug, Error)]
pub enum RunpodError {
    #[error("failed to extract id")]
    NoId(),
    #[error("no gpu configuration with enough memory found")]
    NoGpuConfig(),
}
