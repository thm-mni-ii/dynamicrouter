use crate::config::{BackendConf, BackendImageConf, CONFIG};
use crate::handler::utils::{ModelRef, QuantType};
use once_cell::sync::Lazy;
use std::clone::Clone;
use std::iter::Iterator;
use std::string::ToString;
use std::sync::Arc;

pub type DynBackend = Arc<dyn Backend>;

pub static BACKEND: Lazy<Vec<DynBackend>> = Lazy::new(|| {
    CONFIG
        .backends
        .iter()
        .map(|backend| {
            let backend: DynBackend = match backend {
                BackendConf::Tgi(BackendImageConf { image }) => Arc::new(TgiBackend {
                    image: image.clone(),
                }),
                BackendConf::Vllm(BackendImageConf { image }) => Arc::new(VllmBackend {
                    image: image.clone(),
                }),
                BackendConf::LlamaCpp(BackendImageConf { image }) => Arc::new(LlamaCppBackend {
                    image: image.clone(),
                }),
            };
            backend
        })
        .collect()
});

pub fn get_first_supported_backend(model: &ModelRef) -> Result<DynBackend, BackendError> {
    BACKEND
        .iter()
        .find(|backend| backend.is_supported(model))
        .cloned()
        .ok_or(BackendError::NoSupportedBackedFound())
}

pub trait Backend: Send + Sync {
    fn get_image(&self) -> String;
    fn is_supported(&self, model: &ModelRef) -> bool;
    fn get_args(&self, model: &ModelRef) -> Vec<String>;
    fn get_entrypoint(&self, model: &ModelRef) -> Option<Vec<String>>;
}

pub struct TgiBackend {
    image: String,
}

impl Backend for TgiBackend {
    fn get_image(&self) -> String {
        self.image.clone()
    }

    fn is_supported(&self, model: &ModelRef) -> bool {
        match model.quant {
            None => true,
            Some(quant) => match quant {
                QuantType::FP32 | QuantType::FP16 | QuantType::FP8 | QuantType::FP4 => true,
                QuantType::Gguf(_) => false,
            },
        }
    }

    fn get_args(&self, model: &ModelRef) -> Vec<String> {
        let mut args = Vec::new();
        args.append(&mut vec!["--model-id".to_string(), model.get_hf_id()]);

        if let Some(rev) = &model.rev {
            args.append(&mut vec!["--revision".to_string(), rev.to_string()]);
        }

        match &model.quant {
            Some(QuantType::FP8) => {
                args.append(&mut vec!["--quantize".to_string(), "fp8".to_string()]);
            }
            Some(QuantType::FP4) => args.append(&mut vec![
                "--quantize".to_string(),
                "bitsandbytes-fp4".to_string(),
            ]),
            None => {}
            qt => panic!("quant type {qt:?} not supported by this backend."),
        };

        args
    }

    fn get_entrypoint(&self, _model: &ModelRef) -> Option<Vec<String>> {
        None
    }
}

pub struct VllmBackend {
    image: String,
}

impl Backend for VllmBackend {
    fn get_image(&self) -> String {
        self.image.clone()
    }

    fn is_supported(&self, model: &ModelRef) -> bool {
        match model.quant {
            None => true,
            Some(quant) => match quant {
                QuantType::FP32
                | QuantType::FP16
                | QuantType::FP8
                | QuantType::FP4
                | QuantType::Gguf(_) => true,
            },
        }
    }

    fn get_args(&self, model: &ModelRef) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(QuantType::Gguf(_)) = model.quant {
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
            match &model.quant {
                Some(QuantType::Gguf(_)) => {
                    format!("$MODEL_PATH/{}", model.path.clone().unwrap_or_default())
                }
                _ => model.get_hf_id(),
            },
        ]);

        if let Some(rev) = &model.rev {
            args.append(&mut vec!["--revision".to_string(), rev.to_string()]);
        }

        match &model.quant {
            Some(QuantType::FP8) => {
                args.append(&mut vec!["--quantization".to_string(), "fp8".to_string()]);
            }
            Some(QuantType::FP4) => args.append(&mut vec![
                "--quantization".to_string(),
                "bitsandbytes".to_string(),
            ]),
            Some(QuantType::Gguf(_)) => {
                args.append(&mut vec!["--quantization".to_string(), "gguf".to_string()]);
            }
            None => {}
            qt => panic!("quant type {qt:?} not supported by this backend."),
        };

        match model.quant {
            Some(QuantType::Gguf(_)) => vec!["-c".to_string(), format!("{}", args.join(" "))],
            _ => args,
        }
    }

    fn get_entrypoint(&self, model: &ModelRef) -> Option<Vec<String>> {
        match model.quant {
            Some(QuantType::Gguf(_)) => Some(vec!["/bin/bash".to_string()]),
            _ => None,
        }
    }
}

pub struct LlamaCppBackend {
    image: String,
}

impl Backend for LlamaCppBackend {
    fn get_image(&self) -> String {
        self.image.clone()
    }

    fn is_supported(&self, model: &ModelRef) -> bool {
        matches!(model.quant, Some(QuantType::Gguf(_)))
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
}

#[derive(thiserror::Error, Debug)]
pub enum BackendError {
    #[error("no backend supporting the model could be found")]
    NoSupportedBackedFound(),
}
