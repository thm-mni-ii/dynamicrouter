use crate::backends::{common, Backend};
use crate::config::BackendImageConf;
use crate::utils::{ModelRef, QuantType};
use std::collections::HashMap;

pub struct TgiBackend {
    id: String,
    image: String,
}

impl From<&BackendImageConf> for TgiBackend {
    fn from(value: &BackendImageConf) -> Self {
        TgiBackend {
            id: value.id.clone().unwrap_or_else(|| "llama_cpp".to_string()),
            image: value.image.clone(),
        }
    }
}

impl Backend for TgiBackend {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn get_image(&self) -> String {
        self.image.clone()
    }

    fn is_supported(&self, model: &ModelRef) -> bool {
        match model.get_quant() {
            QuantType::FP32 | QuantType::FP16 | QuantType::FP8 | QuantType::FP4 => true,
            QuantType::Gguf(_) => false,
        }
    }

    fn get_args(&self, model: &ModelRef) -> Vec<String> {
        let mut args = Vec::new();
        args.append(&mut vec!["--model-id".to_string(), model.get_hf_id()]);

        if let Some(rev) = &model.rev {
            args.append(&mut vec!["--revision".to_string(), rev.to_string()]);
        }

        match &model.get_quant() {
            QuantType::FP8 => {
                args.append(&mut vec!["--quantize".to_string(), "fp8".to_string()]);
            }
            QuantType::FP4 => args.append(&mut vec![
                "--quantize".to_string(),
                "bitsandbytes-fp4".to_string(),
            ]),
            QuantType::FP16 => {}
            qt => panic!("quant type {qt:?} not supported by this backends."),
        };

        args
    }

    fn get_entrypoint(&self, _model: &ModelRef) -> Option<Vec<String>> {
        None
    }

    fn get_mount(&self, _model: &ModelRef) -> (String, String) {
        ("models".to_string(), "/models".to_string())
    }

    fn get_environment(&self) -> HashMap<String, String> {
        common::get_environment()
    }
}
