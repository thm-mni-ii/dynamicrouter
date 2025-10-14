use crate::config::{BackendGlobalConf, BackendModelConfigMap, BackendUseFor};
use crate::handlers::spawner::backends::{Backend, common};
use crate::utils::{ModelRef, QuantType};
use std::collections::HashMap;

pub struct LlamaCppBackend {
    id: String,
    image: String,
    use_for: BackendUseFor,
    model_config: BackendModelConfigMap,
}

impl LlamaCppBackend {
    pub fn new(id: &str, value: &BackendGlobalConf) -> Self {
        LlamaCppBackend {
            id: id.to_string(),
            image: value.image.clone(),
            use_for: value.use_for.clone(),
            model_config: value.models.clone(),
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

    fn should_be_used_for(&self, model: &ModelRef) -> bool {
        self.is_supported(model) && self.use_for.should_be_used_for(model.get_quant())
    }

    fn get_args(&self, model: &ModelRef, _gpu_count: u8) -> Vec<String> {
        let mut args = vec![
            "--no-webui".to_string(),
            "--host".to_string(),
            "0.0.0.0".to_string(),
            "--port".to_string(),
            "80".to_string(),
            "--n-gpu-layers".to_string(),
            "999".to_string(),
            "--hf-repo".to_string(),
            model.get_hf_id(),
        ];

        let model_config = self.model_config.get_model(model);
        if let Some(ctx_length) = model_config.ctx_length {
            args.append(&mut vec!["--ctx-size".to_string(), ctx_length.to_string()]);
        }
        args.append(&mut model_config.arguments.clone());

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

    fn get_environment(&self, model: &ModelRef) -> HashMap<String, String> {
        common::get_environment(self.model_config.get_model(model).environment_variables)
    }
}
