use std::collections::HashSet;
use std::process::exit;
use config::Config;
use log::error;
use once_cell::sync::Lazy;
use serde::Deserialize;

fn default_timeout() -> u32 {
    900
}
fn default_retry_interval() -> u32 {
    60
}

#[derive(Deserialize, Debug)]
pub struct Conf {
    pub port: u16,
    pub advertise_models: Vec<String>,
    pub sqlite_url: String,
    #[serde(default = "default_timeout")]
    pub model_timeout: u32,
    #[serde(default = "default_retry_interval")]
    pub model_retry_interval: u32,
    pub hf: HuggingfaceConf,
    pub factory: FactoryConf,
    pub hashes: HashSet<String>,
}

#[derive(Deserialize, Debug)]
pub struct HuggingfaceConf {
    pub token: Option<String>
}

#[derive(Deserialize, Debug)]
pub struct FactoryConf {
    pub runpod: RunpodConf
}

#[derive(Deserialize, Debug)]
pub struct RunpodConf {
    pub token: String,
    pub gpus: Vec<RunpodGpu>,
    pub template_id: String,
    pub image: String,
    pub gpu_memory_margin: Option<u64>,
    pub disk_storage_margin: Option<u64>
}

#[derive(Deserialize, Debug)]
pub struct RunpodGpu {
    pub id: String,
    pub memory: u64,
    pub price: f64
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
