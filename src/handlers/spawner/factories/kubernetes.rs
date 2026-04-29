use crate::config::{CONFIG, KubernetesConf, KubernetesGpu};
use crate::handlers::GenerationType;
use crate::handlers::spawner::backends::get_first_supported_backend;
use crate::handlers::spawner::factories::ModelFactory;
use crate::handlers::spawner::factories::models::{DynModel, HttpModel, Model, ModelError};
use crate::utils::response::ModelResponse;
use crate::utils::{ByteSize, ModelRef, ModelSizes};
use anyhow::Error;
use async_trait::async_trait;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{DeleteParams, PostParams};
use kube::{Api, Client};
use log::{debug, info};
use serde::Serialize;
use serde_json::{Value, json};
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
    async fn create(&self, model: &ModelRef) -> Result<DynModel, anyhow::Error> {
        let config = CONFIG
            .factory
            .as_kubernetes()
            .get_for_model(&model.to_string());

        let model_sizes = ModelSizes::get(model).await?;

        let backend = get_first_supported_backend(model)?;
        let (gpu_id, gpu_count) = calculate_needed_gpu(
            &config,
            model,
            model_sizes.gpu.factor(config.gpu_memory_margin_factor),
        )?;
        info!(
            "Using {gpu_count} x {gpu_id} on backend {} for {model}",
            backend.get_id()
        );
        let id = hex::encode(rand::random::<[u8; 8]>());

        let env = backend
            .get_environment(model)
            .into_iter()
            .map(|(key, value)| json!({"name": key, "value": value}))
            .collect::<Value>();

        let mut node_selector = json!({
            "nvidia.com/gpu.product": gpu_id
        });

        if let Some(additional_node_selector) = &config.additional_node_selector {
            let node_selector_obj = node_selector.as_object_mut().unwrap();
            for (key, value) in additional_node_selector {
                node_selector_obj.insert(key.clone(), json!(value));
            }
        }

        let (sub_path, mount_path) = backend.get_mount(model);

        let mut volumes = vec![
            json!({"name": "models", "persistentVolumeClaim": {"claimName": config.pvc_name}}),
            json!({"name": "dev-shm", "emptyDir": {"medium": "Memory"}}),
        ];

        let mut volume_mounts = vec![
            json!({"name": "models", "subPath": sub_path, "mountPath": mount_path}),
            json!({"name": "dev-shm", "mountPath": "/dev/shm"}),
        ];

        #[allow(clippy::collapsible_if)]
        if let Some(cache_pvc_name) = config.cache_pvc_name {
            if let Some((sub_path, mount_path)) = backend.get_cache_mount(model) {
                volumes.push(json!({"name": "cache", "persistentVolumeClaim": {"claimName": cache_pvc_name}}));
                volume_mounts
                    .push(json!({"name": "cache", "subPath": sub_path, "mountPath": mount_path}));
            }
        }

        self.pod_api
            .create(
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
                            "command": backend.get_entrypoint(model),
                            "args": backend.get_args(model, gpu_count),
                            "volumeMounts": volume_mounts,
                            "env": env,
                            "resources": {
                                "limits": {
                                    "nvidia.com/gpu": gpu_count.to_string(),
                                }
                            }
                        }],
                        "nodeSelector": node_selector,
                        "tolerations": config.additional_tolerations,
                        "volumes": volumes,
                    }
                }))?,
            )
            .await?;

        Ok(KubernetesResolvingModel::new_dyn(
            id,
            model.to_string(),
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

    async fn get_parent(&self) -> Result<&DynModel, ModelError<anyhow::Error>> {
        if let Some(parent) = self.parent.get() {
            return Ok(parent);
        }
        let cr = self
            .api
            .get_opt(&format!("dynamicrouter-inference-{}", self.id))
            .await?;

        let pod =
            cr.ok_or_else(|| ModelError::NonRetryableError(KubernetesError::PodNotFound().into()))?;

        let status = pod.status.as_ref().ok_or_else(KubernetesError::NotReady)?;

        if let Some(scheduled) = status
            .conditions
            .as_ref()
            .and_then(|conditions| conditions.iter().find(|c| c.type_ == "PodScheduled"))
            .filter(|condition| condition.reason.as_deref() == Some("Unschedulable"))
        {
            return Err(ModelError::NonRetryableError(
                KubernetesError::Unschedulable(scheduled.message.clone()).into(),
            ));
        }

        if status.container_statuses.as_ref().is_some_and(|statuses| {
            statuses
                .iter()
                .map(|status| {
                    status
                        .state
                        .as_ref()
                        .and_then(|state| state.waiting.as_ref())
                        .and_then(|state| state.reason.as_ref())
                })
                .any(|reason| reason == Some(&"CrashLoopBackOff".to_string()))
        }) {
            return Err(ModelError::NonRetryableError(
                KubernetesError::Unschedulable("CrashLoopBackOff".to_string().into()).into(),
            ));
        }

        let ip = status.pod_ip.clone().ok_or(KubernetesError::NotReady())?;

        let _ = self.parent.set(HttpModel::new_dyn(
            self.id.clone(),
            self.name.clone(),
            format!("http://{ip}",).parse()?,
            None,
        ));

        Ok(self.parent.get().unwrap())
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

    async fn generate(
        &self,
        generation_type: GenerationType,
        request: &Value,
    ) -> Result<ModelResponse, ModelError<anyhow::Error>> {
        let parent = self.get_parent().await?;
        parent.generate(generation_type, request).await
    }

    async fn invoke(
        &self,
        path: &str,
        body: &Value,
    ) -> Option<Result<ModelResponse, ModelError<anyhow::Error>>> {
        let parent = match self.get_parent().await {
            Ok(parent) => parent,
            Err(e) => return Some(Err(e)),
        };
        parent.invoke(path, body).await
    }

    async fn serialize(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(self)?)
    }
}

#[derive(Debug, Error)]
pub enum KubernetesError {
    #[error("pod does not exist")]
    PodNotFound(),
    #[error("failed to extract pod ip")]
    NotReady(),
    #[error("pod unschedulable: 0")]
    Unschedulable(Option<String>),
    #[error("no gpu configuration with enough memory found")]
    NoGpuConfig(),
    #[error("failed to deserialize")]
    Dezerialize(),
}
