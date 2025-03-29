mod llama_cpp;
mod tgi;
mod vllm;

use crate::backends::llama_cpp::LlamaCppBackend;
use crate::backends::tgi::TgiBackend;
use crate::backends::vllm::VllmBackend;
use crate::config::{BackendConf, CONFIG};
use crate::utils::ModelRef;
use once_cell::sync::Lazy;
use std::sync::Arc;

pub type DynBackend = Arc<dyn Backend>;

pub static BACKEND: Lazy<Vec<DynBackend>> = Lazy::new(|| {
    CONFIG
        .backends
        .iter()
        .map(|backend| {
            let backend: DynBackend = match backend {
                BackendConf::Tgi(backend_image_conf) => {
                    Arc::new(TgiBackend::from(backend_image_conf))
                }
                BackendConf::Vllm(backend_image_conf) => {
                    Arc::new(VllmBackend::from(backend_image_conf))
                }
                BackendConf::LlamaCpp(backend_image_conf) => {
                    Arc::new(LlamaCppBackend::from(backend_image_conf))
                }
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
    fn get_mount(&self, model: &ModelRef) -> (String, String);
}

#[derive(thiserror::Error, Debug)]
pub enum BackendError {
    #[error("no backends supporting the model could be found")]
    NoSupportedBackedFound(),
}
