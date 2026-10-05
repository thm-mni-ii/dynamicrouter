mod common;
mod llama_cpp;
mod sglang;
mod tgi;
mod vllm;

use crate::db::backend::{get_all_backends, get_backend_model_config};
use crate::handlers::spawner::backends::llama_cpp::LlamaCppBackend;
use crate::handlers::spawner::backends::sglang::SglangBackend;
use crate::handlers::spawner::backends::tgi::TgiBackend;
use crate::handlers::spawner::backends::vllm::VllmBackend;
use crate::utils::ModelRef;
use log::debug;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub type DynBackend = Arc<dyn Backend>;

pub async fn get_first_supported_backend(
    model: &ModelRef,
    allowed_backends: Option<&HashSet<String>>,
) -> Result<DynBackend, BackendError> {
    let all_backends = get_all_backends()
        .await
        .map_err(|_| BackendError::NoSupportedBackedFound())?;

    for backend_rec in all_backends {
        if let Some(allowed) = allowed_backends
            && !allowed.contains(&backend_rec.id)
        {
            continue;
        }

        let model_config = get_backend_model_config(&backend_rec.id, &model.to_string())
            .await
            .unwrap_or(None)
            .unwrap_or_default();

        let backend: DynBackend = match backend_rec.backend_type.as_str() {
            "llama_cpp" => Arc::new(LlamaCppBackend::new(
                &backend_rec.id,
                &backend_rec.image,
                backend_rec.use_for,
                model_config,
            )),
            "sglang" => Arc::new(SglangBackend::new(
                &backend_rec.id,
                &backend_rec.image,
                backend_rec.use_for,
                model_config,
            )),
            "vllm" => Arc::new(VllmBackend::new(
                &backend_rec.id,
                &backend_rec.image,
                backend_rec.use_for,
                model_config,
            )),
            "tgi" => Arc::new(TgiBackend::new(
                &backend_rec.id,
                &backend_rec.image,
                backend_rec.use_for,
                model_config,
            )),
            _ => continue,
        };

        if backend.should_be_used_for(model) {
            debug!("Selected backend {} for model {}", backend.get_id(), model);
            return Ok(backend);
        }
    }

    Err(BackendError::NoSupportedBackedFound())
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
