use crate::config::{BackendGlobalConf, BackendModelConfigMap, BackendUseFor};
use crate::handlers::spawner::backends::{Backend, common};
use crate::utils::{ModelRef, QuantType};
use std::collections::HashMap;

pub struct TgiBackend {
    id: String,
    image: String,
    use_for: BackendUseFor,
    model_config: BackendModelConfigMap,
}

impl TgiBackend {
    pub fn new(id: &str, value: &BackendGlobalConf) -> Self {
        TgiBackend {
            id: id.to_string(),
            image: value.image.clone(),
            use_for: value.use_for.clone(),
            model_config: value.models.clone(),
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
            QuantType::Gguf(_) | QuantType::Awq => false,
        }
    }

    fn should_be_used_for(&self, model: &ModelRef) -> bool {
        self.is_supported(model) && self.use_for.should_be_used_for(model.get_quant())
    }

    fn get_args(&self, model: &ModelRef, _gpu_count: u8) -> Vec<String> {
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
        }

        let model_config = self.model_config.get_model(model);
        args.append(&mut model_config.arguments.clone());

        args
    }

    fn get_entrypoint(&self, _model: &ModelRef) -> Option<Vec<String>> {
        None
    }

    fn get_mount(&self, _model: &ModelRef) -> (String, String) {
        ("models".to_string(), "/models".to_string())
    }

    fn get_environment(&self, model: &ModelRef) -> HashMap<String, String> {
        common::get_environment(self.model_config.get_model(model).environment_variables)
    }
}
