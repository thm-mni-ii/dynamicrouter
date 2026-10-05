use crate::config::{CONFIG, Handler as ConfigHandler, OpenAiApiConf, OpenAiModelHandlerConfig};
use crate::db::instance::ensure_instance_exists;
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
    api_conf: OpenAiApiConf,
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

    fn resolve_target(model: &ModelRef) -> Result<ResolvedTarget, anyhow::Error> {
        let model_str = model.to_string();

        // 1. Check if model explicitly configured in CONFIG.models
        if let Some(model_conf) = CONFIG.models.get(&model_str)
            && let ConfigHandler::OpenAi(ref openai_conf) = model_conf.handler
        {
            return Self::resolve_from_model_handler(model, openai_conf);
        }

        // 2. Check if wildcard model configured with OpenAI handler
        if let Some(wildcard_conf) = CONFIG.models.get("*")
            && let ConfigHandler::OpenAi(ref openai_conf) = wildcard_conf.handler
        {
            return Self::resolve_from_model_handler(model, openai_conf);
        }

        // 3. Match against configured openai_apis by prefix or explicit models list
        for (api_name, api_conf) in &CONFIG.openai_apis {
            if api_conf.matches_model(&model_str) {
                let target_model = api_conf.map_model_name(&model_str);
                return Ok(ResolvedTarget {
                    api_name: api_name.clone(),
                    api_conf: api_conf.clone(),
                    target_model,
                    timeout: api_conf.timeout,
                    headers: api_conf.headers.clone(),
                });
            }
        }

        // 4. If only one API is configured in openai_apis, use it as fallback
        if CONFIG.openai_apis.len() == 1 {
            let (api_name, api_conf) = CONFIG.openai_apis.iter().next().unwrap();
            let target_model = api_conf.map_model_name(&model_str);
            return Ok(ResolvedTarget {
                api_name: api_name.clone(),
                api_conf: api_conf.clone(),
                target_model,
                timeout: api_conf.timeout,
                headers: api_conf.headers.clone(),
            });
        }

        anyhow::bail!("No matching OpenAI API found for model {model_str}")
    }

    fn resolve_from_model_handler(
        model: &ModelRef,
        handler_conf: &OpenAiModelHandlerConfig,
    ) -> Result<ResolvedTarget, anyhow::Error> {
        let model_str = model.to_string();

        if let Some(api_name) = &handler_conf.api {
            let api_conf = CONFIG.openai_apis.get(api_name).ok_or_else(|| {
                anyhow::anyhow!("OpenAI API '{api_name}' not found in configuration")
            })?;
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
                api_conf: api_conf.clone(),
                target_model,
                timeout,
                headers,
            });
        }

        if let Some(url) = &handler_conf.url {
            let inline_api = OpenAiApiConf {
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

        // Search openai_apis by matching prefix/model
        for (api_name, api_conf) in &CONFIG.openai_apis {
            if api_conf.matches_model(&model_str) {
                let target_model = handler_conf
                    .model
                    .clone()
                    .unwrap_or_else(|| api_conf.map_model_name(&model_str));
                let timeout = handler_conf.timeout.or(api_conf.timeout);
                return Ok(ResolvedTarget {
                    api_name: api_name.clone(),
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

        if CONFIG.openai_apis.len() == 1 {
            let (api_name, api_conf) = CONFIG.openai_apis.iter().next().unwrap();
            let target_model = handler_conf
                .model
                .clone()
                .unwrap_or_else(|| api_conf.map_model_name(&model_str));
            let timeout = handler_conf.timeout.or(api_conf.timeout);
            return Ok(ResolvedTarget {
                api_name: api_name.clone(),
                api_conf: api_conf.clone(),
                target_model,
                timeout,
                headers: handler_conf
                    .headers
                    .clone()
                    .or_else(|| api_conf.headers.clone()),
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
    ) -> Result<ModelResponse, anyhow::Error> {
        let ResolvedTarget {
            api_name,
            api_conf,
            target_model,
            timeout: timeout_opt,
            headers: headers_opt,
        } = Self::resolve_target(model)?;

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

        let logged_request_body = if CONFIG.should_log_request_body_for_user(username) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::init_db;
    use axum::Json;
    use axum::Router;
    use axum::routing::post;
    use futures_util::StreamExt;
    use serde_json::json;
    use url::Url;

    async fn mock_chat_completions(Json(body): Json<Value>) -> Json<Value> {
        let requested_model = body.get("model").and_then(|m| m.as_str()).unwrap_or("");
        Json(json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "model": requested_model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello from mock OpenAI endpoint!"
                },
                "finish_reason": "stop"
            }]
        }))
    }

    #[tokio::test]
    async fn test_openai_handler_inline_and_forwarding() {
        init_db().await;

        let mock_app = Router::new().route("/v1/chat/completions", post(mock_chat_completions));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, mock_app).await.unwrap();
        });

        let mock_url = Url::parse(&format!("http://127.0.0.1:{port}/v1")).unwrap();
        let handler_conf = OpenAiModelHandlerConfig {
            api: None,
            url: Some(mock_url),
            token: None,
            model: Some("gpt-4o-custom".to_string()),
            timeout: Some(30),
            headers: None,
        };

        let target = OpenaiHandler::resolve_from_model_handler(
            &"my-custom-model".parse().unwrap(),
            &handler_conf,
        )
        .unwrap();
        assert_eq!(target.target_model, "gpt-4o-custom");
        assert_eq!(target.api_name, "inline");

        let api_conf = target.api_conf;
        let completions_url = api_conf.endpoint_url("chat/completions").unwrap();
        assert_eq!(
            completions_url.as_str(),
            format!("http://127.0.0.1:{port}/v1/chat/completions")
        );
    }

    #[tokio::test]
    async fn test_openai_handler_stream_and_logging() {
        init_db().await;

        let mock_app = Router::new().route("/v1/chat/completions", post(mock_chat_completions));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, mock_app).await.unwrap();
        });

        let target_url =
            Url::parse(&format!("http://127.0.0.1:{port}/v1/chat/completions")).unwrap();
        let client = reqwest::Client::new();
        ensure_instance_exists("openai:test", "test-model")
            .await
            .unwrap();
        let request_id = create_request("test-model", "openai:test", "default", None)
            .await
            .unwrap();

        let res = client
            .post(target_url)
            .json(&json!({"model": "test-model"}))
            .send()
            .await
            .unwrap();

        let model_response = ModelResponse {
            status: res.status(),
            content_type: res
                .headers()
                .get("Content-Type")
                .and_then(|ct| ct.to_str().ok())
                .map(ToString::to_string),
            content_length: res.content_length(),
            request_id: None,
            body: Box::new(StreamAdapter::new(res.bytes_stream())),
        };

        let mut wrapped = wrap_response_with_logging(model_response, request_id.clone());
        assert_eq!(wrapped.request_id, Some(request_id.clone()));

        let mut accumulated_bytes = Vec::new();
        while let Some(chunk) = wrapped.body.next().await {
            accumulated_bytes.extend_from_slice(&chunk.unwrap());
        }

        let body_str = String::from_utf8(accumulated_bytes).unwrap();
        assert!(body_str.contains("Hello from mock OpenAI endpoint!"));

        request_done(&request_id).await.unwrap();
    }
}
