use std::process::exit;
use config::Config;
use log::error;
use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Conf {
    pub port: u16,
    pub runpod_token: String,
    pub advertise_models: Vec<String>,
    pub sqlite_url: String,
    pub model_timeout: u32
}

pub static CONFIG: Lazy<Conf> = Lazy::new(|| {
    let conf = Config::builder()
        .add_source(config::File::with_name("config.toml"))
        .add_source(config::Environment::default())
        .build()
        .unwrap();

    conf.try_deserialize().unwrap_or_else(|err| {
        error!("Failed to parse config: {err}");
        exit(1)
    })
});
