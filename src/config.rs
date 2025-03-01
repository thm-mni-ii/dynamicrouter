use std::collections::HashSet;
use std::process::exit;
use config::Config;
use log::error;
use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Conf {
    pub port: u16,
    pub advertise_models: Vec<String>,
    pub sqlite_url: String,
    pub model_timeout: u32,
    pub huggingface_token: Option<String>,
    pub factory: FactoryConf,
    pub hashes: HashSet<String>,
}

#[derive(Deserialize)]
pub struct FactoryConf {
    pub runpod: RunpodConf
}

#[derive(Deserialize)]
pub struct RunpodConf {
    pub token: String,
    pub gpus: Vec<RunpodGpu>,
    pub template_id: String,
    pub image: String,
}

#[derive(Deserialize)]
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
