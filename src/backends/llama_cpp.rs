use crate::backends::Backend;
use crate::config::BackendImageConf;
use crate::utils::{ModelRef, QuantType};

pub struct LlamaCppBackend {
    image: String,
}

impl From<&BackendImageConf> for LlamaCppBackend {
    fn from(value: &BackendImageConf) -> Self {
        LlamaCppBackend {
            image: value.image.clone(),
        }
    }
}

impl Backend for LlamaCppBackend {
    fn get_image(&self) -> String {
        self.image.clone()
    }

    fn is_supported(&self, model: &ModelRef) -> bool {
        matches!(model.get_quant(), QuantType::Gguf(_))
    }

    fn get_args(&self, model: &ModelRef) -> Vec<String> {
        let mut args = vec![
            "--no-webui".to_string(),
            "--port".to_string(),
            "80".to_string(),
            "--n-gpu-layers".to_string(),
            "99".to_string(),
            "--hf-repo".to_string(),
            model.get_hf_id(),
        ];

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
}
