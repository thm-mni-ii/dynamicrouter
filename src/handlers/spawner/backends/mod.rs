mod common;
mod llama_cpp;
mod sglang;
mod tgi;
mod vllm;

use crate::config::{BackendConf, CONFIG, Handler, SpawnerModelHandlerConfig};
use crate::handlers::spawner::backends::llama_cpp::LlamaCppBackend;
use crate::handlers::spawner::backends::sglang::SglangBackend;
use crate::handlers::spawner::backends::tgi::TgiBackend;
use crate::handlers::spawner::backends::vllm::VllmBackend;
use crate::utils::ModelRef;
use log::debug;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;

pub type DynBackend = Arc<dyn Backend>;

pub static BACKEND: Lazy<Vec<DynBackend>> = Lazy::new(|| {
    CONFIG
        .backends
        .iter()
        .map(|(id, backend)| {
            let backend: DynBackend = match backend {
                BackendConf::Tgi(backend_image_conf) => {
                    Arc::new(TgiBackend::new(id, backend_image_conf))
                }
                BackendConf::Vllm(backend_image_conf) => {
                    Arc::new(VllmBackend::new(id, backend_image_conf))
                }
                BackendConf::Sglang(backend_image_conf) => {
                    Arc::new(SglangBackend::new(id, backend_image_conf))
                }
                BackendConf::LlamaCpp(backend_image_conf) => {
                    Arc::new(LlamaCppBackend::new(id, backend_image_conf))
                }
            };
            backend
        })
        .collect()
});

pub fn get_first_supported_backend(model: &ModelRef) -> Result<DynBackend, BackendError> {
    let per_model_backends =
        CONFIG
            .models
            .get(&model.to_string())
            .and_then(|model| match &model.handler {
                Handler::Spawner(SpawnerModelHandlerConfig { backends, .. }) => backends.as_ref(),
            });

    let mut backend_iter: Box<dyn Iterator<Item = &DynBackend>> = Box::new(BACKEND.iter());

    if let Some(per_model_backends) = per_model_backends {
        debug!("{model} limits backends to {per_model_backends:?}");
        backend_iter =
            Box::new(backend_iter.filter(|backend| per_model_backends.contains(&backend.get_id())));
    } else {
        backend_iter = Box::new(backend_iter.filter(|backend| backend.should_be_used_for(model)));
    }

    backend_iter
        .next()
        .cloned()
        .ok_or(BackendError::NoSupportedBackedFound())
}

pub trait Backend: Send + Sync {
    fn get_id(&self) -> String;
    fn get_image(&self) -> String;
    fn is_supported(&self, model: &ModelRef) -> bool;
    fn should_be_used_for(&self, model: &ModelRef) -> bool;
    fn get_args(&self, model: &ModelRef, gpu_count: u8) -> Vec<String>;
    fn get_entrypoint(&self, model: &ModelRef) -> Option<Vec<String>>;
    fn get_mount(&self, model: &ModelRef) -> (String, String);
    fn get_cache_mount(&self, model: &ModelRef) -> Option<(String, String)>;
    fn get_environment(&self, model: &ModelRef) -> HashMap<String, String>;
}

#[derive(thiserror::Error, Debug)]
pub enum BackendError {
    #[error("no backends supporting the model could be found")]
    NoSupportedBackedFound(),
}
