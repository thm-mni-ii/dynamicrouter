use crate::config::{BackendGlobalConf, BackendModelConfigMap, BackendUseFor};
use crate::handlers::spawner::backends::{Backend, common};
use crate::utils::{ModelRef, QuantType};
use std::collections::HashMap;

pub struct VllmBackend {
    id: String,
    image: String,
    use_for: BackendUseFor,
    model_config: BackendModelConfigMap,
}

impl VllmBackend {
    pub fn new(id: &str, value: &BackendGlobalConf) -> Self {
        VllmBackend {
            id: id.to_string(),
            image: value.image.clone(),
            use_for: value.use_for.clone(),
            model_config: value.models.clone(),
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
        let mut args = vec![
            "--port".to_string(),
            "80".to_string(),
            "--tensor-parallel-size".to_string(),
            gpu_count.to_string(),
        ];
        if let QuantType::Gguf(_) = model.get_quant() {
            args.append(&mut vec![
                "export".to_string(),
                "MODEL_PATH=$(huggingface-cli".to_string(),
                "download".to_string(),
                model.get_hf_id(),
                "--include".to_string(),
                format!("{})", model.path.clone().unwrap_or_default()),
            ]);
            if let Some(rev) = &model.rev {
                args.append(&mut vec!["--revision".to_string(), rev.to_string()]);
            }
            args.append(&mut vec![
                "&&".to_string(),
                "python3".to_string(),
                "-m".to_string(),
                "vllm.entrypoints.openai.api_server".to_string(),
            ]);
        }

        args.append(&mut vec![
            "--model".to_string(),
            match &model.get_quant() {
                QuantType::Gguf(_) => {
                    format!("$MODEL_PATH/{}", model.path.clone().unwrap_or_default())
                }
                _ => model.get_hf_id(),
            },
        ]);

        if let Some(rev) = &model.rev {
            args.append(&mut vec!["--revision".to_string(), rev.to_string()]);
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
            qt => panic!("quant type {qt:?} not supported by this backends."),
        }

        let model_config = self.model_config.get_model(model);
        if let Some(ctx_length) = model_config.ctx_length {
            args.append(&mut vec![
                "--max-model-len".to_string(),
                ctx_length.to_string(),
            ]);
        }
        args.append(&mut model_config.arguments.clone());

        match model.get_quant() {
            QuantType::Gguf(_) => vec!["-c".to_string(), format!("{}", args.join(" "))],
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

    fn get_environment(&self, model: &ModelRef) -> HashMap<String, String> {
        common::get_environment(self.model_config.get_model(model).environment_variables)
    }
}
