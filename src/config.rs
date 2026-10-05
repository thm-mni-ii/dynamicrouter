#![allow(unreachable_patterns)]

use crate::utils::{ModelRef, QuantType};
use config::Config;
use log::error;
use once_cell::sync::Lazy;
use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::process::exit;
use url::Url;

fn default_timeout() -> u32 {
    900
}
fn default_retry_interval() -> u32 {
    60
}
fn default_log_request_body() -> bool {
    false
}
fn default_gpu_memory_margin_factor() -> f64 {
    1.2
}
fn default_disk_storage_margin_factor() -> f64 {
    2.0
}
fn default_aliases() -> HashMap<String, Alias> {
    HashMap::new()
}
fn default_handler() -> Handler {
    Handler::Spawner(SpawnerModelHandlerConfig {
        backends: None,
        min_scale: default_min_scale(),
        max_scale: default_max_scale(),
        queue_length_for_scale_up: default_queue_length_for_scale_up(),
    })
}
fn default_min_scale() -> u32 {
    0
}
fn default_max_scale() -> u32 {
    1
}
fn default_queue_length_for_scale_up() -> u32 {
    1
}

#[derive(Deserialize, Debug)]
pub struct Conf {
    pub port: u16,
    pub tls: Option<TlsConfig>,
    pub models: HashMap<String, Model>,
    pub sqlite_url: String,
    #[serde(default = "default_timeout")]
    pub model_timeout: u32,
    #[serde(default = "default_retry_interval")]
    pub model_retry_interval: u32,
    #[serde(default = "default_log_request_body")]
    pub log_request_body: bool,
    #[serde(default)]
    pub openai_apis: HashMap<String, OpenAiApiConf>,
    pub huggingface: HuggingfaceConf,
    pub factory: FactoryConf,
    pub backends: HashMap<String, BackendConf>,
    pub users: HashMap<String, User>,
    #[serde(default = "default_aliases")]
    pub aliases: HashMap<String, Alias>,
}

#[derive(Deserialize, Debug)]
pub struct Model {
    pub name: Option<String>,
    pub timeout: Option<u32>,
    pub limit_to_users: Option<HashSet<String>>,
    pub hidden: Option<bool>,
    #[serde(default = "default_handler")]
    pub handler: Handler,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Handler {
    #[serde(rename = "spawner")]
    Spawner(SpawnerModelHandlerConfig),
    #[serde(rename = "openai")]
    OpenAi(OpenAiModelHandlerConfig),
}

#[derive(Deserialize, Debug, Default)]
pub struct OpenAiModelHandlerConfig {
    pub api: Option<String>,
    pub url: Option<Url>,
    pub token: Option<SecretBox<String>>,
    pub model: Option<String>,
    pub timeout: Option<u32>,
    pub headers: Option<HashMap<String, String>>,
}

impl Clone for OpenAiModelHandlerConfig {
    fn clone(&self) -> Self {
        OpenAiModelHandlerConfig {
            api: self.api.clone(),
            url: self.url.clone(),
            token: self
                .token
                .as_ref()
                .map(|t| SecretBox::new(Box::new(t.expose_secret().clone()))),
            model: self.model.clone(),
            timeout: self.timeout,
            headers: self.headers.clone(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct OpenAiApiConf {
    pub url: Url,
    pub token: Option<SecretBox<String>>,
    pub prefix: Option<String>,
    pub models: Option<HashSet<String>>,
    pub timeout: Option<u32>,
    pub headers: Option<HashMap<String, String>>,
}

impl Clone for OpenAiApiConf {
    fn clone(&self) -> Self {
        OpenAiApiConf {
            url: self.url.clone(),
            token: self
                .token
                .as_ref()
                .map(|t| SecretBox::new(Box::new(t.expose_secret().clone()))),
            prefix: self.prefix.clone(),
            models: self.models.clone(),
            timeout: self.timeout,
            headers: self.headers.clone(),
        }
    }
}

impl OpenAiApiConf {
    pub fn endpoint_url(&self, path: &str) -> Result<Url, url::ParseError> {
        let mut base = self.url.as_str().trim_end_matches('/').to_string();
        if !base.ends_with("/v1") && !base.contains("/v1/") {
            base.push_str("/v1");
        }
        let clean_path = path.trim_start_matches('/');
        let url_str = format!("{base}/{clean_path}");
        Url::parse(&url_str)
    }

    pub fn map_model_name(&self, model_name: &str) -> String {
        if let Some(prefix) = &self.prefix {
            let p = prefix.trim_end_matches('/');
            if let Some(stripped) = model_name.strip_prefix(prefix.as_str()) {
                stripped.trim_start_matches('/').to_string()
            } else if let Some(stripped) = model_name.strip_prefix(p) {
                stripped.trim_start_matches('/').to_string()
            } else {
                model_name.to_string()
            }
        } else {
            model_name.to_string()
        }
    }

    pub fn matches_model(&self, model_name: &str) -> bool {
        if let Some(prefix) = &self.prefix {
            let p = prefix.trim_end_matches('/');
            if model_name.starts_with(prefix.as_str())
                || model_name.starts_with(&format!("{p}/"))
                || model_name == p
            {
                return true;
            }
        }
        if let Some(models) = &self.models {
            if models.contains(model_name) {
                return true;
            }
            if self.prefix.is_some() {
                let stripped = self.map_model_name(model_name);
                if models.contains(&stripped) {
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpawnerModelHandlerConfig {
    pub backends: Option<HashSet<String>>,
    #[serde(default = "default_min_scale")]
    pub min_scale: u32,
    #[serde(default = "default_max_scale")]
    pub max_scale: u32,
    #[serde(default = "default_queue_length_for_scale_up")]
    pub queue_length_for_scale_up: u32,
}

impl Handler {
    pub fn as_spawner(&self) -> &SpawnerModelHandlerConfig {
        match self {
            Handler::Spawner(conf) => conf,
            _ => panic!("model handler config is not spawner"),
        }
    }

    #[allow(dead_code)]
    pub fn as_openai(&self) -> &OpenAiModelHandlerConfig {
        match self {
            Handler::OpenAi(conf) => conf,
            _ => panic!("model handler config is not openai"),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub token_hashes: HashSet<String>,
    pub limit_to_models: Option<HashSet<String>>,
    pub log_request_body: Option<bool>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum TlsConfig {
    TlsPathConfig { cert_path: String, key_path: String },
    TlsInlineConfig { cert: String, key: String },
}

#[derive(Deserialize, Debug)]
pub struct HuggingfaceConf {
    pub token: Option<SecretBox<String>>,
}

#[derive(Deserialize, Debug)]
pub enum FactoryConf {
    #[serde(rename = "runpod")]
    Runpod(RunpodConf),
    #[serde(rename = "kubernetes")]
    Kubernetes(KubernetesConf),
}

impl FactoryConf {
    pub fn as_runpod(&self) -> &RunpodConf {
        match self {
            FactoryConf::Runpod(conf) => conf,
            _ => panic!("factory is not runpod"),
        }
    }
    pub fn as_kubernetes(&self) -> &KubernetesConf {
        match self {
            FactoryConf::Kubernetes(conf) => conf,
            _ => panic!("factory is not kubernetes"),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct RunpodConf {
    pub token: SecretBox<String>,
    pub gpus: Vec<RunpodGpu>,
    pub template_id: String,
    #[serde(default = "default_gpu_memory_margin_factor")]
    pub gpu_memory_margin_factor: f64,
    #[serde(default = "default_disk_storage_margin_factor")]
    pub disk_storage_margin_factor: f64,
    pub models: Option<HashMap<String, RunpodModel>>,
}

impl RunpodConf {
    pub fn get_for_model(&self, name: &str) -> RunpodConf {
        let model_specific = self.models.as_ref().and_then(|models| models.get(name));
        RunpodConf {
            token: SecretBox::new(Box::new(self.token.expose_secret().clone())),
            gpus: model_specific
                .and_then(|ms| ms.gpus.as_ref())
                .unwrap_or(&self.gpus)
                .clone(),
            template_id: model_specific
                .and_then(|ms| ms.template_id.as_ref())
                .unwrap_or(&self.template_id)
                .clone(),
            gpu_memory_margin_factor: model_specific
                .and_then(|ms| ms.gpu_memory_margin)
                .unwrap_or(self.gpu_memory_margin_factor),
            disk_storage_margin_factor: model_specific
                .and_then(|ms| ms.disk_storage_margin)
                .unwrap_or(self.disk_storage_margin_factor),
            models: None,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct RunpodGpu {
    pub id: String,
    pub memory: u64,
    pub price: f64,
}

#[derive(Deserialize, Debug)]
pub struct RunpodModel {
    template_id: Option<String>,
    gpu_memory_margin: Option<f64>,
    disk_storage_margin: Option<f64>,
    gpus: Option<Vec<RunpodGpu>>,
}

#[derive(Deserialize, Debug)]
pub struct KubernetesConf {
    pub namespace: String,
    pub pvc_name: String,
    pub cache_pvc_name: Option<String>,
    pub gpus: Vec<KubernetesGpu>,
    #[serde(default = "default_gpu_memory_margin_factor")]
    pub gpu_memory_margin_factor: f64,
    pub additional_node_selector: Option<HashMap<String, String>>,
    pub additional_tolerations: Option<Vec<KubernetesToleration>>,
    pub models: Option<HashMap<String, KubernetesModel>>,
}

impl KubernetesConf {
    pub fn get_for_model(&self, name: &str) -> KubernetesConf {
        let model_specific = self.models.as_ref().and_then(|models| models.get(name));
        KubernetesConf {
            namespace: self.namespace.clone(),
            pvc_name: model_specific
                .and_then(|ms| ms.pvc_name.as_ref())
                .unwrap_or(&self.pvc_name)
                .clone(),
            cache_pvc_name: model_specific
                .and_then(|ms| ms.cache_pvc_name.as_ref())
                .or(self.cache_pvc_name.as_ref())
                .cloned(),
            gpus: model_specific
                .and_then(|ms| ms.gpus.as_ref())
                .unwrap_or(&self.gpus)
                .clone(),
            gpu_memory_margin_factor: model_specific
                .and_then(|ms| ms.gpu_memory_margin_factor)
                .unwrap_or(self.gpu_memory_margin_factor),
            additional_node_selector: model_specific
                .and_then(|ms| ms.additional_node_selector.as_ref())
                .or(self.additional_node_selector.as_ref())
                .cloned(),
            additional_tolerations: model_specific
                .and_then(|ms| ms.additional_tolerations.as_ref())
                .or(self.additional_tolerations.as_ref())
                .cloned(),
            models: None,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct KubernetesGpu {
    pub id: String,
    pub memory: u64,
    pub max_count: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct KubernetesToleration {
    pub key: String,
    pub operator: String,
    pub value: String,
    pub effect: String,
}

#[derive(Deserialize, Debug)]
pub struct KubernetesModel {
    pub pvc_name: Option<String>,
    pub cache_pvc_name: Option<String>,
    pub gpus: Option<Vec<KubernetesGpu>>,
    pub gpu_memory_margin_factor: Option<f64>,
    pub additional_node_selector: Option<HashMap<String, String>>,
    pub additional_tolerations: Option<Vec<KubernetesToleration>>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum BackendConf {
    #[serde(rename = "tgi")]
    Tgi(BackendGlobalConf),
    #[serde(rename = "vllm")]
    Vllm(BackendGlobalConf),
    #[serde(rename = "llama_cpp")]
    LlamaCpp(BackendGlobalConf),
    #[serde(rename = "sglang")]
    Sglang(BackendGlobalConf),
}

#[derive(Deserialize, Debug)]
pub struct BackendGlobalConf {
    pub image: String,
    #[serde(default)]
    pub use_for: BackendUseFor,
    #[serde(default)]
    pub models: BackendModelConfigMap,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct BackendModelConfigMap(HashMap<String, BackendModelConf>);

impl BackendModelConfigMap {
    pub fn get_model(&self, model_ref: &ModelRef) -> BackendModelConf {
        self.0
            .get(&model_ref.to_string())
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct BackendUseFor(Option<HashSet<QuantType>>);

impl BackendUseFor {
    pub fn should_be_used_for(&self, quant_type: QuantType) -> bool {
        match &self.0 {
            None => true,
            Some(quants) => quants.contains(&quant_type),
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct BackendModelConf {
    #[serde(default)]
    pub environment_variables: HashMap<String, String>,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub ctx_length: Option<u64>,
}

pub static CONFIG: Lazy<Conf> = Lazy::new(|| {
    let conf = Config::builder()
        .add_source(config::File::with_name("config.toml"))
        .add_source(config::Environment::default().separator("_"))
        .build()
        .unwrap();

    conf.try_deserialize().unwrap_or_else(|err| {
        error!("Failed to parse config: {err}");
        exit(1)
    })
});

pub fn get_model_config(model: &ModelRef) -> Result<&'static Model, ModelNotFoundError> {
    get_model_config_by_name(&model.to_string())
}

pub fn get_model_config_by_name(model: &str) -> Result<&'static Model, ModelNotFoundError> {
    CONFIG
        .models
        .get(model)
        .or_else(|| CONFIG.models.get("*"))
        .ok_or_else(|| ModelNotFoundError {
            id: model.to_string(),
        })
}

#[derive(thiserror::Error, Debug)]
#[error("model {id} not found")]
pub struct ModelNotFoundError {
    id: String,
}

#[derive(Deserialize, Debug)]
pub struct Alias {
    pub to: String,
}

impl Conf {
    pub fn should_log_request_body_for_user(&self, username: &str) -> bool {
        self.users
            .get(username)
            .and_then(|u| u.log_request_body)
            .unwrap_or(self.log_request_body)
    }
}
