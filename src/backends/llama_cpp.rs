use crate::backends::{common, Backend};
use crate::config::LlamaCppConf;
use crate::utils::{ModelRef, QuantType};
use std::collections::HashMap;

pub struct LlamaCppBackend {
    id: String,
    image: String,
    ctx_length: Option<u64>,
}

impl From<&LlamaCppConf> for LlamaCppBackend {
    fn from(value: &LlamaCppConf) -> Self {
        LlamaCppBackend {
            id: value.id.clone().unwrap_or_else(|| "llama_cpp".to_string()),
            image: value.image.clone(),
            ctx_length: value.ctx_length,
        }
    }
}

impl Backend for LlamaCppBackend {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn get_image(&self) -> String {
        self.image.clone()
    }

    fn is_supported(&self, model: &ModelRef) -> bool {
        matches!(model.get_quant(), QuantType::Gguf(_))
    }

    fn get_args(&self, model: &ModelRef, _gpu_count: u8) -> Vec<String> {
        let mut args = vec![
            "--no-webui".to_string(),
            "--port".to_string(),
            "80".to_string(),
            "--n-gpu-layers".to_string(),
            "99".to_string(),
            "--hf-repo".to_string(),
            model.get_hf_id(),
        ];
        
        if let Some(ctx_length) = self.ctx_length {
            args.append(&mut vec!["--ctx-size".to_string(), ctx_length.to_string()]);
        }

        if let Some(path) = &model.path {
            args.append(&mut vec!["--hf-file".to_string(), path.clone()]);
        }

        args
    }

    fn get_entrypoint(&self, _model: &ModelRef) -> Option<Vec<String>> {
        None
    }

    fn get_mount(&self, _model: &ModelRef) -> (String, String) {
        (
            "llama.cpp".to_string(),
            "/root/.cache/llama.cpp/".to_string(),
        )
    }

    fn get_environment(&self) -> HashMap<String, String> {
        common::get_environment()
    }
}
