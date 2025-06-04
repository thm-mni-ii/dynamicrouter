use crate::config::CONFIG;
use secrecy::ExposeSecret;
use std::collections::HashMap;

pub fn get_environment() -> HashMap<String, String> {
    [
        CONFIG
            .huggingface
            .token
            .as_ref()
            .map(|ht| ("HF_TOKEN".to_string(), ht.expose_secret().to_string())),
        Some(("HF_HOME".to_string(), "/models".to_string())),
    ]
    .into_iter()
    .flatten()
    .collect()
}
