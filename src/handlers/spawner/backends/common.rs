use crate::config::CONFIG;
use crate::utils::ModelRef;
use secrecy::ExposeSecret;
use std::collections::HashMap;

pub fn get_environment(additional: HashMap<String, String>) -> HashMap<String, String> {
    [
        CONFIG
            .huggingface_token
            .as_ref()
            .map(|ht| ("HF_TOKEN".to_string(), ht.expose_secret().clone())),
        Some(("HF_HOME".to_string(), "/models".to_string())),
    ]
    .into_iter()
    .flatten()
    .chain(additional)
    .collect()
}

pub fn get_gguf_download_command(model: &ModelRef) -> Vec<String> {
    let mut args = Vec::new();
    args.append(&mut vec![
        "export".to_string(),
        "MODEL_PATH=$(hf".to_string(),
        "download".to_string(),
        model.get_hf_id(),
        "--include".to_string(),
        format!("{})", model.path.clone().unwrap_or_default()),
    ]);
    if let Some(rev) = &model.rev {
        args.append(&mut vec!["--revision".to_string(), rev.clone()]);
    }
    args.push("&&".to_string());
    args
}
