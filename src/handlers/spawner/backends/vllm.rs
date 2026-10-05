use crate::db::backend::{BackendModelRecord, BackendUseFor};
use crate::handlers::spawner::backends::common::get_gguf_download_command;
use crate::handlers::spawner::backends::{Backend, common};
use crate::utils::{ModelRef, QuantType};
use std::collections::HashMap;

pub struct VllmBackend {
    id: String,
    image: String,
    use_for: BackendUseFor,
    model_config: BackendModelRecord,
}

impl VllmBackend {
    pub fn new(
        id: &str,
        image: &str,
        use_for: BackendUseFor,
        model_config: BackendModelRecord,
    ) -> Self {
        VllmBackend {
            id: id.to_string(),
            image: image.to_string(),
            use_for,
            model_config,
        }
    }
}

impl Backend for VllmBackend {
    fn get_id(&self) -> String {
        self.id.clone()
    }
    fn get_image(&self) -> String {
        self.image.clone()
    }

    fn is_supported(&self, model: &ModelRef) -> bool {
        match model.get_quant() {
            QuantType::FP32
            | QuantType::FP16
            | QuantType::FP8
            | QuantType::FP4
            | QuantType::Awq
            | QuantType::Gguf(_) => true,
        }
    }

    fn should_be_used_for(&self, model: &ModelRef) -> bool {
        self.is_supported(model) && self.use_for.should_be_used_for(model.get_quant())
    }

    fn get_args(&self, model: &ModelRef, gpu_count: u8) -> Vec<String> {
        let mut args = if let QuantType::Gguf(_) = model.get_quant() {
            let mut args = get_gguf_download_command(model);
            args.append(&mut vec![
                "python3".to_string(),
                "-m".to_string(),
                "vllm.entrypoints.openai.api_server".to_string(),
            ]);
            args
        } else {
            Vec::new()
        };
        args.append(&mut vec![
            "--port".to_string(),
            "80".to_string(),
            "--tensor-parallel-size".to_string(),
            gpu_count.to_string(),
        ]);

        args.append(&mut vec![match &model.get_quant() {
            QuantType::Gguf(_) => {
                format!("$MODEL_PATH/{}", model.path.clone().unwrap_or_default())
            }
            _ => model.get_hf_id(),
        }]);

        if let Some(rev) = &model.rev {
            args.append(&mut vec!["--revision".to_string(), rev.clone()]);
        }

        match &model.get_quant() {
            QuantType::FP8 => {
                args.append(&mut vec!["--quantization".to_string(), "fp8".to_string()]);
            }
            QuantType::FP4 => args.append(&mut vec![
                "--quantization".to_string(),
                "bitsandbytes".to_string(),
            ]),
            QuantType::Awq => {
                args.append(&mut vec!["--quantization".to_string(), "awq".to_string()]);
            }
            QuantType::Gguf(_) => {
                args.append(&mut vec!["--quantization".to_string(), "gguf".to_string()]);
            }
            QuantType::FP16 => {}
            qt => panic!("quant type {qt:?} not supported by this backend."),
        }

        if let Some(ctx_length) = self.model_config.ctx_length {
            args.append(&mut vec![
                "--max-model-len".to_string(),
                ctx_length.to_string(),
            ]);
        }
        args.append(&mut self.model_config.arguments.clone());

        match model.get_quant() {
            QuantType::Gguf(_) => vec!["-c".to_string(), args.join(" ")],
            _ => args,
        }
    }

    fn get_entrypoint(&self, model: &ModelRef) -> Option<Vec<String>> {
        match model.get_quant() {
            QuantType::Gguf(_) => Some(vec!["/bin/bash".to_string()]),
            _ => None,
        }
    }

    fn get_mount(&self, _model: &ModelRef) -> (String, String) {
        ("models".to_string(), "/models".to_string())
    }

    fn get_cache_mount(&self, _model: &ModelRef) -> Option<(String, String)> {
        Some(("vllm".to_string(), "/root/.cache/vllm".to_string()))
    }

    fn get_environment(&self, _model: &ModelRef) -> HashMap<String, String> {
        common::get_environment(self.model_config.environment_variables.clone())
    }
}
