use crate::db::model::{ModelHandlerConfig, get_exact_model_config, get_model_config};
use crate::db::openai_api::get_all_openai_apis;
use crate::handlers::openai_api::OpenaiHandler;
use crate::handlers::spawner::SpawnerHandler;
use crate::utils::ModelRef;
use crate::utils::response::ModelResponse;
use anyhow::Error;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

pub mod openai_api;
pub mod spawner;

static RH: OnceCell<RoutingHandler> = OnceCell::new();

pub async fn init_handlers() {
    if RH.get().is_none() {
        let rh = RoutingHandler::new().await;
        let _ = RH.set(rh);
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
        user_log_request_body: Option<bool>,
        prompt_tokens: i64,
        token_hash: Option<&str>,
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
        user_log_request_body: Option<bool>,
        prompt_tokens: i64,
        token_hash: Option<&str>,
    ) -> Result<ModelResponse, Error> {
        let model_str = model.to_string();

        // 1. Check if model is explicitly configured in DB models
        if let Some(model_conf) = get_exact_model_config(&model_str).await? {
            return match &model_conf.handler {
                ModelHandlerConfig::Spawner(_) => {
                    self.spawner_handler
                        .generate(
                            generation_type,
                            model,
                            request,
                            username,
                            user_log_request_body,
                            prompt_tokens,
                            token_hash,
                        )
                        .await
                }
                ModelHandlerConfig::OpenAi(_) => {
                    self.openai_handler
                        .generate(
                            generation_type,
                            model,
                            request,
                            username,
                            user_log_request_body,
                            prompt_tokens,
                            token_hash,
                        )
                        .await
                }
            };
        }

        // 2. Match against configured openai_apis by prefix or explicit models list
        let openai_apis = get_all_openai_apis().await?;
        for api_conf in &openai_apis {
            if api_conf.matches_model(&model_str) {
                return self
                    .openai_handler
                    .generate(
                        generation_type,
                        model,
                        request,
                        username,
                        user_log_request_body,
                        prompt_tokens,
                        token_hash,
                    )
                    .await;
            }
        }

        // 3. Match against glob pattern models or wildcard '*' model in models table
        if let Some(model_conf) = get_model_config(&model_str).await? {
            return match &model_conf.handler {
                ModelHandlerConfig::Spawner(_) => {
                    self.spawner_handler
                        .generate(
                            generation_type,
                            model,
                            request,
                            username,
                            user_log_request_body,
                            prompt_tokens,
                            token_hash,
                        )
                        .await
                }
                ModelHandlerConfig::OpenAi(_) => {
                    self.openai_handler
                        .generate(
                            generation_type,
                            model,
                            request,
                            username,
                            user_log_request_body,
                            prompt_tokens,
                            token_hash,
                        )
                        .await
                }
            };
        }

        // 4. Fallback if single openai API is configured
        if openai_apis.len() == 1 {
            return self
                .openai_handler
                .generate(
                    generation_type,
                    model,
                    request,
                    username,
                    user_log_request_body,
                    prompt_tokens,
                    token_hash,
                )
                .await;
        }

        anyhow::bail!("Model '{model_str}' not found")
    }
}
