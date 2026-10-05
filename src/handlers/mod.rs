use crate::config;
use crate::config::get_model_config;
use crate::handlers::openai_api::OpenaiHandler;
use crate::handlers::spawner::SpawnerHandler;
use crate::utils::ModelRef;
use crate::utils::response::ModelResponse;
use anyhow::Error;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

mod openai_api;
mod spawner;

static RH: OnceCell<RoutingHandler> = OnceCell::new();

pub async fn init_handlers() {
    let rh = RoutingHandler::new().await;
    match RH.set(rh) {
        Ok(()) => {}
        Err(_) => panic!("init_handlers should only be called once"),
    }
}

pub fn get_handler() -> &'static RoutingHandler {
    RH.get()
        .expect("you tried to access the handler before it was initialized")
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub enum GenerationType {
    Completions,
    Embeddings,
}

#[async_trait::async_trait]
pub trait Handler: Send + Sync {
    async fn generate(
        &self,
        generation_type: GenerationType,
        model: &ModelRef,
        request: &Value,
        username: &str,
    ) -> Result<ModelResponse, anyhow::Error>;
}

pub type DynHandler = Arc<dyn Handler>;

pub struct RoutingHandler {
    spawner_handler: DynHandler,
    openai_handler: DynHandler,
}

impl RoutingHandler {
    async fn new() -> Self {
        RoutingHandler {
            spawner_handler: SpawnerHandler::new_dyn().await,
            openai_handler: OpenaiHandler::new_dyn().await,
        }
    }
}

#[async_trait::async_trait]
impl Handler for RoutingHandler {
    async fn generate(
        &self,
        generation_type: GenerationType,
        model: &ModelRef,
        request: &Value,
        username: &str,
    ) -> Result<ModelResponse, Error> {
        let model_str = model.to_string();

        if let Some(model_conf) = config::CONFIG.models.get(&model_str) {
            return match &model_conf.handler {
                config::Handler::Spawner { .. } => {
                    self.spawner_handler
                        .generate(generation_type, model, request, username)
                        .await
                }
                config::Handler::OpenAi { .. } => {
                    self.openai_handler
                        .generate(generation_type, model, request, username)
                        .await
                }
            };
        }

        for api_conf in config::CONFIG.openai_apis.values() {
            if api_conf.matches_model(&model_str) {
                return self
                    .openai_handler
                    .generate(generation_type, model, request, username)
                    .await;
            }
        }

        let model_conf = get_model_config(model)?;
        match &model_conf.handler {
            config::Handler::Spawner { .. } => {
                self.spawner_handler
                    .generate(generation_type, model, request, username)
                    .await
            }
            config::Handler::OpenAi { .. } => {
                self.openai_handler
                    .generate(generation_type, model, request, username)
                    .await
            }
        }
    }
}
