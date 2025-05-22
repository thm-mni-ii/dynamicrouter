use config::Config;
use log::error;
use once_cell::sync::Lazy;
use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::process::exit;

fn default_timeout() -> u32 {
    900
}
fn default_retry_interval() -> u32 {
    60
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
    pub huggingface: HuggingfaceConf,
    pub factory: FactoryConf,
    pub backends: Vec<BackendConf>,
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
    pub backends: Option<HashSet<String>>,
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub token_hashes: HashSet<String>,
    pub limit_to_models: Option<HashSet<String>>,
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
            FactoryConf::Kubernetes(_) => panic!("factory is not runpod"),
        }
    }
    pub fn as_kubernetes(&self) -> &KubernetesConf {
        match self {
            FactoryConf::Kubernetes(conf) => conf,
            FactoryConf::Runpod(_) => panic!("factory is not kubernetes"),
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
    pub gpus: Option<Vec<KubernetesGpu>>,
    pub gpu_memory_margin_factor: Option<f64>,
    pub additional_node_selector: Option<HashMap<String, String>>,
    pub additional_tolerations: Option<Vec<KubernetesToleration>>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum BackendConf {
    #[serde(rename = "tgi")]
    Tgi(BackendImageConf),
    #[serde(rename = "vllm")]
    Vllm(BackendImageConf),
    #[serde(rename = "llama_cpp")]
    LlamaCpp(LlamaCppConf),
}

#[derive(Deserialize, Debug)]
pub struct BackendImageConf {
    pub image: String,
    pub id: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct LlamaCppConf {
    pub image: String,
    pub id: Option<String>,
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

#[derive(Deserialize, Debug)]
pub struct Alias {
    pub to: String,
}
