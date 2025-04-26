use crate::backends::{common, Backend};
use crate::config::BackendImageConf;
use crate::utils::{ModelRef, QuantType};
use std::collections::HashMap;

pub struct VllmBackend {
    id: String,
    image: String,
}

impl From<&BackendImageConf> for VllmBackend {
    fn from(value: &BackendImageConf) -> Self {
        VllmBackend {
            id: value.id.clone().unwrap_or_else(|| "vllm".to_string()),
            image: value.image.clone(),
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
            | QuantType::Gguf(_) => true,
        }
    }

    fn get_args(&self, model: &ModelRef) -> Vec<String> {
        let mut args = Vec::new();
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
            QuantType::Gguf(_) => {
                args.append(&mut vec!["--quantization".to_string(), "gguf".to_string()]);
            }
            QuantType::FP16 => {}
            qt => panic!("quant type {qt:?} not supported by this backends."),
        };

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

    fn get_environment(&self) -> HashMap<String, String> {
        common::get_environment()
    }
}
