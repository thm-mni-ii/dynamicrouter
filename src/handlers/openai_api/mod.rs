use crate::config::CONFIG;
use crate::db::instance::ensure_instance_exists;
use crate::db::model::{ModelHandlerConfig, OpenAiHandlerConfig, get_model_config};
use crate::db::openai_api::{OpenAiApiRecord, get_all_openai_apis, get_openai_api};
use crate::db::request::{create_request, request_done};
use crate::handlers::{DynHandler, GenerationType, Handler};
use crate::utils::ModelRef;
use crate::utils::response::{ModelResponse, StreamAdapter, wrap_response_with_logging};
use log::error;
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub struct OpenaiHandler {
    client: reqwest::Client,
}

struct ResolvedTarget {
    api_name: String,
    api_conf: OpenAiApiRecord,
    target_model: String,
    timeout: Option<u32>,
    headers: Option<HashMap<String, String>>,
}

impl OpenaiHandler {
    #[allow(clippy::unused_async)]
    pub async fn new_dyn() -> DynHandler {
        Arc::new(OpenaiHandler {
            client: reqwest::Client::new(),
        })
    }

    async fn resolve_target(model: &ModelRef) -> Result<ResolvedTarget, anyhow::Error> {
        let model_str = model.to_string();

        // 1. Check if model explicitly configured in DB models
        if let Some(model_conf) = get_model_config(&model_str).await?
            && let ModelHandlerConfig::OpenAi(ref openai_conf) = model_conf.handler
        {
            return Self::resolve_from_model_handler(model, openai_conf).await;
        }

        let openai_apis = get_all_openai_apis().await?;

        // 2. Match against configured openai_apis by prefix or explicit models list
        for api_conf in &openai_apis {
            if api_conf.matches_model(&model_str) {
                let target_model = api_conf.map_model_name(&model_str);
                return Ok(ResolvedTarget {
                    api_name: api_conf.id.clone(),
                    api_conf: api_conf.clone(),
                    target_model,
                    timeout: api_conf.timeout,
                    headers: api_conf.headers.clone(),
                });
            }
        }

        // 3. If only one API is configured in openai_apis, use it as fallback
        if openai_apis.len() == 1 {
            let api_conf = openai_apis.into_iter().next().unwrap();
            let target_model = api_conf.map_model_name(&model_str);
            return Ok(ResolvedTarget {
                api_name: api_conf.id.clone(),
                api_conf,
                target_model,
                timeout: None,
                headers: None,
            });
        }

        anyhow::bail!("No matching OpenAI API found for model {model_str}")
    }

    async fn resolve_from_model_handler(
        model: &ModelRef,
        handler_conf: &OpenAiHandlerConfig,
    ) -> Result<ResolvedTarget, anyhow::Error> {
        let model_str = model.to_string();

        if let Some(api_name) = &handler_conf.api {
            let api_conf = get_openai_api(api_name)
                .await?
                .ok_or_else(|| anyhow::anyhow!("OpenAI API '{api_name}' not found in database"))?;
            let target_model = handler_conf
                .model
                .clone()
                .unwrap_or_else(|| api_conf.map_model_name(&model_str));
            let timeout = handler_conf.timeout.or(api_conf.timeout);
            let headers = match (&api_conf.headers, &handler_conf.headers) {
                (None, None) => None,
                (Some(h), None) | (None, Some(h)) => Some(h.clone()),
                (Some(h1), Some(h2)) => {
                    let mut merged = h1.clone();
                    merged.extend(h2.clone());
                    Some(merged)
                }
            };
            return Ok(ResolvedTarget {
                api_name: api_name.clone(),
                api_conf,
                target_model,
                timeout,
                headers,
            });
        }

        if let Some(url) = &handler_conf.url {
            let inline_api = OpenAiApiRecord {
                id: "inline".to_string(),
                url: url.clone(),
                token: handler_conf
                    .token
                    .as_ref()
                    .map(|t| secrecy::SecretBox::new(Box::new(t.expose_secret().clone()))),
                prefix: None,
                models: None,
                timeout: handler_conf.timeout,
                headers: handler_conf.headers.clone(),
            };
            let target_model = handler_conf
                .model
                .clone()
                .unwrap_or_else(|| model.repo.clone());
            return Ok(ResolvedTarget {
                api_name: "inline".to_string(),
                api_conf: inline_api,
                target_model,
                timeout: handler_conf.timeout,
                headers: handler_conf.headers.clone(),
            });
        }

        let openai_apis = get_all_openai_apis().await?;

        // Search openai_apis by matching prefix/model
        for api_conf in &openai_apis {
            if api_conf.matches_model(&model_str) {
                let target_model = handler_conf
                    .model
                    .clone()
                    .unwrap_or_else(|| api_conf.map_model_name(&model_str));
                let timeout = handler_conf.timeout.or(api_conf.timeout);
                return Ok(ResolvedTarget {
                    api_name: api_conf.id.clone(),
                    api_conf: api_conf.clone(),
                    target_model,
                    timeout,
                    headers: handler_conf
                        .headers
                        .clone()
                        .or_else(|| api_conf.headers.clone()),
                });
            }
        }

        if openai_apis.len() == 1 {
            let api_conf = openai_apis.into_iter().next().unwrap();
            let target_model = handler_conf
                .model
                .clone()
                .unwrap_or_else(|| api_conf.map_model_name(&model_str));
            let timeout = handler_conf.timeout.or(api_conf.timeout);
            return Ok(ResolvedTarget {
                api_name: api_conf.id.clone(),
                api_conf,
                target_model,
                timeout,
                headers: handler_conf.headers.clone(),
            });
        }

        anyhow::bail!("OpenAI handler for model {model_str} did not specify an API or URL")
    }
}

#[async_trait::async_trait]
impl Handler for OpenaiHandler {
    async fn generate(
        &self,
        generation_type: GenerationType,
        model: &ModelRef,
        request: &Value,
        username: &str,
        user_log_request_body: Option<bool>,
    ) -> Result<ModelResponse, anyhow::Error> {
        let ResolvedTarget {
            api_name,
            api_conf,
            target_model,
            timeout: timeout_opt,
            headers: headers_opt,
        } = Self::resolve_target(model).await?;

        let endpoint_path = match generation_type {
            GenerationType::Completions => "chat/completions",
            GenerationType::Embeddings => "embeddings",
        };

        let target_url = api_conf.endpoint_url(endpoint_path)?;
        let timeout_secs = timeout_opt.unwrap_or(CONFIG.model_timeout);

        let mut request_body = request.clone();
        if let Some(obj) = request_body.as_object_mut() {
            obj.insert("model".to_string(), json!(target_model));
        }

        let model_name = model.to_string();
        let instance_name = format!("openai:{api_name}");
        ensure_instance_exists(&instance_name, &model_name).await?;

        let logged_request_body = if CONFIG.should_log_request_body_for_user(user_log_request_body)
        {
            Some(&request_body)
        } else {
            None
        };

        let request_id =
            create_request(&model_name, &instance_name, username, logged_request_body).await?;

        let mut req_builder = self
            .client
            .post(target_url)
            .json(&request_body)
            .timeout(Duration::from_secs(u64::from(timeout_secs)));

        if let Some(token) = &api_conf.token {
            req_builder = req_builder.bearer_auth(token.expose_secret());
        }

        if let Some(headers) = headers_opt {
            for (k, v) in headers {
                req_builder = req_builder.header(k, v);
            }
        }

        let response_result = req_builder.send().await;
        request_done(&request_id).await?;

        match response_result {
            Ok(res) => {
                let status = res.status();
                let content_type = res
                    .headers()
                    .get("Content-Type")
                    .and_then(|ct| ct.to_str().ok())
                    .map(ToString::to_string);
                let content_length = res.content_length();

                let model_response = ModelResponse {
                    status,
                    content_type,
                    content_length,
                    request_id: None,
                    body: Box::new(StreamAdapter::new(res.bytes_stream())),
                };

                Ok(wrap_response_with_logging(model_response, request_id))
            }
            Err(err) => {
                error!("Error while sending request to OpenAI API '{api_name}': {err}");
                Err(err.into())
            }
        }
    }
}
