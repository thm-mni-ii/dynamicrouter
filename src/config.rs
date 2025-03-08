use config::Config;
use log::error;
use once_cell::sync::Lazy;
use secrecy::SecretBox;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::process::exit;

fn default_timeout() -> u32 {
    900
}
fn default_retry_interval() -> u32 {
    60
}
fn default_gpu_memory_margin() -> u64 {
    5
}
fn default_disk_storage_margin() -> u64 {
    50
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
    pub users: HashMap<String, User>,
}

#[derive(Deserialize, Debug)]
pub struct Model {
    pub name: Option<String>,
    pub timeout: Option<u32>,
    pub limit_to_users: Option<HashSet<String>>,
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub token_hashes: HashSet<String>,
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
    pub image: String,
    #[serde(default = "default_gpu_memory_margin")]
    pub gpu_memory_margin: u64,
    #[serde(default = "default_disk_storage_margin")]
    pub disk_storage_margin: u64,
}

#[derive(Deserialize, Debug)]
pub struct RunpodGpu {
    pub id: String,
    pub memory: u64,
    pub price: f64,
}

#[derive(Deserialize, Debug)]
pub struct KubernetesConf {
    pub namespace: String,
    pub image: String,
    pub pvc_name: String,
    pub gpus: Vec<KubernetesGpu>,
    #[serde(default = "default_gpu_memory_margin")]
    pub gpu_memory_margin: u64,
    pub additional_node_selector: Option<HashMap<String, String>>,
    pub additional_tolerations: Option<Vec<KubernetesToleration>>,
}

#[derive(Deserialize, Debug)]
pub struct KubernetesGpu {
    pub id: String,
    pub memory: u64,
    pub max_count: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct KubernetesToleration {
    pub key: String,
    pub operator: String,
    pub value: String,
    pub effect: String,
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
