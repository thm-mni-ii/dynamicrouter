use crate::config::{KubernetesConf, KubernetesGpu, CONFIG};
use crate::handler::backend::get_first_supported_backend;
use crate::handler::factories::ModelFactory;
use crate::handler::models::{DynModel, HttpModel, Model, ModelResponse};
use crate::handler::utils::{ByteSize, ModelRef, ModelSizes};
use anyhow::Error;
use async_trait::async_trait;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{DeleteParams, PostParams};
use kube::{Api, Client};
use log::{debug, info};
use secrecy::ExposeSecret;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::OnceCell;

pub struct KubernetesModelFactory {
    pod_api: Api<Pod>,
}

impl KubernetesModelFactory {
    pub async fn new() -> Result<KubernetesModelFactory, anyhow::Error> {
        Ok(KubernetesModelFactory {
            pod_api: Api::namespaced(
                Client::try_default().await?,
                &CONFIG.factory.as_kubernetes().namespace,
            ),
        })
    }
}

pub fn calculate_gpu_match(gpu: &KubernetesGpu, needed: ByteSize) -> Option<(String, u8)> {
    let gpu_size = ByteSize::from_gibibyte(gpu.memory);
    let mut gpus_needed = (needed.bytes() / gpu_size.bytes()) + 1;
    if gpus_needed != 1 && gpus_needed % 2 == 1 {
        gpus_needed += 1;
    }
    if gpus_needed <= gpu.max_count.unwrap_or(u64::MAX) {
        Some((gpu.id.clone(), gpus_needed as u8))
    } else {
        None
    }
}

fn calculate_needed_gpu(
    config: &KubernetesConf,
    model: &ModelRef,
    needed_memory: ByteSize,
) -> Result<(String, u8), anyhow::Error> {
    let mut options = config
        .gpus
        .iter()
        .filter_map(|gpu| calculate_gpu_match(gpu, needed_memory))
        .collect::<Vec<(String, u8)>>();
    options.sort_by_key(|a| a.1);

    debug!("Valid options for {model:?}: {options:?}");

    Ok(options
        .first()
        .map(|(model, count)| (model.clone(), *count))
        .ok_or(KubernetesError::NoGpuConfig())?)
}

#[async_trait]
impl ModelFactory for KubernetesModelFactory {
    async fn create(&self, model: ModelRef) -> Result<DynModel, anyhow::Error> {
        let config = CONFIG
            .factory
            .as_kubernetes()
            .get_for_model(&model.to_string());

        let model_sizes = ModelSizes::get(&model).await?;

        let (gpu_id, gpu_count) = calculate_needed_gpu(
            &config,
            &model,
            model_sizes.gpu.factor(config.gpu_memory_margin_factor),
        )?;
        info!("Using {gpu_count} x {gpu_id} for {model}");
        let id = hex::encode(rand::random::<[u8; 8]>());

        let mut env = json!([{"name": "HF_HOME", "value": "/models"}]);

        if let Some(token) = &CONFIG.huggingface.token {
            env.as_array_mut()
                .unwrap()
                .push(json!({"name": "HF_TOKEN", "value": token.expose_secret()}));
        }

        let mut node_selector = json!({
            "nvidia.com/gpu.product": gpu_id
        });

        if let Some(additional_node_selector) = &config.additional_node_selector {
            let node_selector_obj = node_selector.as_object_mut().unwrap();
            for (key, value) in additional_node_selector {
                node_selector_obj.insert(key.to_string(), json!(value));
            }
        }

        let backend = get_first_supported_backend(&model)?;

        self.pod_api.create(
            &PostParams::default(),
            &serde_json::from_value(json!({
    "apiVersion": "v1",
    "kind": "Pod",
    "metadata": {
        "name": format!("dynamicrouter-inference-{id}")
    },
    "spec": {
        "containers": [{
            "name": "inference",
            "image": backend.get_image(),
            "command": backend.get_entrypoint(&model),
            "args": backend.get_args(&model),
            "volumeMounts": [
                {"name": "models", "mountPath": "/models"}
            ],
            "env": env,
            "resources": {
                "limits": {
                    "nvidia.com/gpu": gpu_count.to_string(),
                }
            }
        }],
        "nodeSelector": node_selector,
        "tolerations": config.additional_tolerations,
        "volumes": [
            {"name": "models", "persistentVolumeClaim": {"claimName": config.pvc_name}}
        ]
    }
}))?,
        ).await?;

        Ok(KubernetesResolvingModel::new_dyn(
            id,
            model.get_hf_id(),
            self.pod_api.clone(),
        ))
    }

    async fn destroy(&self, model: DynModel) -> Result<(), anyhow::Error> {
        self.pod_api
            .delete(
                &format!("dynamicrouter-inference-{}", model.id().await),
                &DeleteParams::default(),
            )
            .await?;
        Ok(())
    }

    async fn dump(&self, model: &DynModel) -> Result<Vec<u8>, anyhow::Error> {
        model.serialize().await
    }

    async fn restore(&self, dump: &[u8]) -> Result<DynModel, anyhow::Error> {
        let parsed = serde_json::from_slice::<Value>(dump)?;
        Ok(KubernetesResolvingModel::new_dyn(
            parsed
                .get("id")
                .and_then(|id| id.as_str())
                .map(std::string::ToString::to_string)
                .ok_or(KubernetesError::Dezerialize())?,
            parsed
                .get("name")
                .and_then(|name| name.as_str())
                .map(std::string::ToString::to_string)
                .ok_or(KubernetesError::Dezerialize())?,
            self.pod_api.clone(),
        ))
    }
}

#[derive(Serialize)]
struct KubernetesResolvingModel {
    id: String,
    name: String,
    #[serde(skip)]
    parent: OnceCell<DynModel>,
    #[serde(skip)]
    api: Api<Pod>,
}

impl KubernetesResolvingModel {
    pub fn new_dyn(id: String, name: String, api: Api<Pod>) -> DynModel {
        Arc::new(KubernetesResolvingModel {
            id,
            name,
            parent: OnceCell::default(),
            api,
        })
    }
}

#[async_trait]
impl Model for KubernetesResolvingModel {
    async fn id(&self) -> &str {
        &self.id
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn generate(&self, request: &Value) -> Result<ModelResponse, Error> {
        if let Some(parent) = self.parent.get() {
            return parent.generate(request).await;
        }
        let cr = self
            .api
            .get(&format!("dynamicrouter-inference-{}", self.id))
            .await?;

        let ip = cr
            .status
            .and_then(|s| s.pod_ip)
            .ok_or(KubernetesError::NoIP())?;

        let _ = self.parent.set(HttpModel::new_dyn(
            self.id.clone(),
            self.name.clone(),
            format!("http://{ip}/v1/chat/completions",).parse()?,
            None,
        ));

        self.generate(request).await
    }

    async fn serialize(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(self)?)
    }
}

#[derive(Debug, Error)]
pub enum KubernetesError {
    #[error("failed to extract pod ip")]
    NoIP(),
    #[error("no gpu configuration with enough memory found")]
    NoGpuConfig(),
    #[error("failed to deserialize")]
    Dezerialize(),
}
