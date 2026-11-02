use crate::models::*;
use reqwest::{Client, Response};
use serde::{de::DeserializeOwned, Deserialize};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::window;

pub fn get_origin() -> String {
    window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_else(|| "".to_string())
}

pub fn get_stored_token() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("dr_api_key").ok().flatten())
}

pub fn set_stored_token(token: Option<&str>) {
    if let Some(w) = window().and_then(|w| w.local_storage().ok().flatten()) {
        if let Some(t) = token {
            let _ = w.set_item("dr_api_key", t);
        } else {
            let _ = w.remove_item("dr_api_key");
        }
    }
}

fn client() -> Client {
    Client::new()
}

fn apply_auth(req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    if let Some(token) = get_stored_token() {
        if !token.trim().is_empty() {
            return req.bearer_auth(token.trim());
        }
    }
    req
}

async fn handle_json_response<T: DeserializeOwned>(res: Response) -> Result<T, String> {
    let status = res.status();
    if status.is_success() {
        res.json::<T>()
            .await
            .map_err(|e| format!("JSON parse error: {e}"))
    } else {
        let text = res.text().await.unwrap_or_default();
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(err_val) = json_val.get("error") {
                if let Some(msg) = err_val.as_str() {
                    return Err(format!("Error ({status}): {msg}"));
                } else if let Some(msg) = err_val.get("message").and_then(|m| m.as_str()) {
                    return Err(format!("Error ({status}): {msg}"));
                }
            }
        }
        let trimmed = text.trim();
        if trimmed.starts_with("<!DOCTYPE") || trimmed.starts_with("<html") {
            return Err(format!("HTTP {status}: Upstream returned an HTML error page"));
        }
        Err(format!("HTTP {status}: {text}"))
    }
}

// --------------------------------------------------------------------------
// Auth & Profile API
// --------------------------------------------------------------------------
pub async fn fetch_auth_config() -> Result<AuthConfig, String> {
    let url = format!("{}/auth/config", get_origin());
    let res = client().get(&url).send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
}

pub async fn fetch_current_user() -> Result<UserProfile, String> {
    let url = format!("{}/api/me", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
}

pub async fn logout() -> Result<(), String> {
    let url = format!("{}/auth/logout", get_origin());
    let req = apply_auth(client().post(&url));
    let _ = req.send().await;
    set_stored_token(None);
    Ok(())
}

// --------------------------------------------------------------------------
// Models List (/v1/models)
// --------------------------------------------------------------------------
pub async fn fetch_v1_models() -> Result<Vec<ModelListItem>, String> {
    let url = format!("{}/v1/models", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let body = handle_json_response::<ModelsListResponse>(res).await?;
    Ok(body.data)
}

// --------------------------------------------------------------------------
// Chat Completions (/v1/chat/completions)
// --------------------------------------------------------------------------
pub async fn send_chat_completion(
    request: &ChatCompletionRequest,
) -> Result<ChatCompletionResponse, String> {
    let url = format!("{}/v1/chat/completions", get_origin());
    let req = apply_auth(client().post(&url).json(request));
    let res = req.send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
}

pub async fn send_chat_completion_stream<F>(
    request: &ChatCompletionRequest,
    mut on_chunk: F,
) -> Result<(), String>
where
    F: FnMut(&str),
{
    let origin = get_origin();
    let url = format!("{origin}/v1/chat/completions");

    let window = web_sys::window().ok_or_else(|| "No global window found".to_string())?;

    let body_json = serde_json::to_string(request)
        .map_err(|e| format!("Failed to serialize request: {e}"))?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&wasm_bindgen::JsValue::from_str(&body_json));

    let headers = web_sys::Headers::new()
        .map_err(|e| format!("Failed to create headers: {e:?}"))?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("Failed to set Content-Type header: {e:?}"))?;
    headers
        .set("Accept", "text/event-stream")
        .map_err(|e| format!("Failed to set Accept header: {e:?}"))?;

    if let Some(token) = get_stored_token() {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            headers
                .set("Authorization", &format!("Bearer {trimmed}"))
                .map_err(|e| format!("Failed to set Authorization header: {e:?}"))?;
        }
    }

    opts.set_headers(&headers);

    let web_req = web_sys::Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| format!("Failed to create request: {e:?}"))?;

    let resp_val = JsFuture::from(window.fetch_with_request(&web_req))
        .await
        .map_err(|e| format!("Fetch failed: {e:?}"))?;

    let resp: web_sys::Response = resp_val
        .dyn_into()
        .map_err(|_| "Response cast failed".to_string())?;

    let status = resp.status();
    if !resp.ok() {
        let text_val = JsFuture::from(
            resp.text()
                .map_err(|e| format!("Failed to read error body: {e:?}"))?,
        )
        .await
        .map_err(|e| format!("Failed to await error body: {e:?}"))?;

        let text = text_val.as_string().unwrap_or_default();
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(err_val) = json_val.get("error") {
                if let Some(msg) = err_val.as_str() {
                    return Err(format!("Error ({status}): {msg}"));
                } else if let Some(msg) = err_val.get("message").and_then(|m| m.as_str()) {
                    return Err(format!("Error ({status}): {msg}"));
                }
            }
        }
        let trimmed = text.trim();
        if trimmed.starts_with("<!DOCTYPE") || trimmed.starts_with("<html") {
            return Err(format!("HTTP {status}: Upstream returned an HTML error page"));
        }
        return Err(format!("HTTP {status}: {text}"));
    }

    let stream = resp
        .body()
        .ok_or_else(|| "Response has no body".to_string())?;
    let reader: web_sys::ReadableStreamDefaultReader = stream
        .get_reader()
        .dyn_into()
        .map_err(|_| "Failed to get stream reader".to_string())?;

    let mut byte_buffer: Vec<u8> = Vec::new();
    let mut text_buffer = String::new();

    loop {
        let read_promise = reader.read();
        let chunk_res = JsFuture::from(read_promise)
            .await
            .map_err(|e| format!("Stream read error: {e:?}"))?;

        let done = js_sys::Reflect::get(&chunk_res, &wasm_bindgen::JsValue::from_str("done"))
            .map_err(|e| format!("Reflect error on 'done': {e:?}"))?
            .as_bool()
            .unwrap_or(false);

        if done {
            break;
        }

        let value = js_sys::Reflect::get(&chunk_res, &wasm_bindgen::JsValue::from_str("value"))
            .map_err(|e| format!("Reflect error on 'value': {e:?}"))?;

        if !value.is_undefined() && !value.is_null() {
            let uint8_arr = js_sys::Uint8Array::new(&value);
            byte_buffer.extend_from_slice(&uint8_arr.to_vec());
        }

        // Safely extract valid UTF-8 up to valid boundary
        match std::str::from_utf8(&byte_buffer) {
            Ok(s) => {
                text_buffer.push_str(s);
                byte_buffer.clear();
            }
            Err(e) => {
                let valid_up_to = e.valid_up_to();
                if valid_up_to > 0 {
                    if let Ok(s) = std::str::from_utf8(&byte_buffer[..valid_up_to]) {
                        text_buffer.push_str(s);
                    }
                    byte_buffer.drain(..valid_up_to);
                }
                if let Some(err_len) = e.error_len() {
                    // Invalid byte sequence encountered; skip it
                    byte_buffer.drain(..err_len);
                }
            }
        }

        // Process any complete SSE lines in text_buffer
        while let Some(pos) = text_buffer.find('\n') {
            let line = text_buffer[..pos].trim_end_matches('\r').to_string();
            text_buffer.drain(..=pos);

            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(':') {
                continue;
            }

            if let Some(data) = trimmed.strip_prefix("data:") {
                let data = data.trim();
                if data == "[DONE]" {
                    continue;
                }
                if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                    for choice in chunk.choices {
                        let text_val = choice
                            .delta
                            .content
                            .as_deref()
                            .or(choice.delta.reasoning_content.as_deref())
                            .or(choice.delta.text.as_deref())
                            .or(choice.text.as_deref());

                        if let Some(content) = text_val {
                            if !content.is_empty() {
                                on_chunk(content);
                            }
                        }
                    }
                    if let Some(r) = chunk.response {
                        if !r.is_empty() {
                            on_chunk(&r);
                        }
                    }
                }
            }
        }
    }

    // Process any remaining data in text_buffer after EOF
    if !byte_buffer.is_empty() {
        let s = String::from_utf8_lossy(&byte_buffer);
        text_buffer.push_str(&s);
    }
    let trimmed = text_buffer.trim();
    if !trimmed.is_empty() {
        if let Some(data) = trimmed.strip_prefix("data:") {
            let data = data.trim();
            if data != "[DONE]" {
                if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                    for choice in chunk.choices {
                        let text_val = choice
                            .delta
                            .content
                            .as_deref()
                            .or(choice.delta.reasoning_content.as_deref())
                            .or(choice.delta.text.as_deref())
                            .or(choice.text.as_deref());

                        if let Some(content) = text_val {
                            if !content.is_empty() {
                                on_chunk(content);
                            }
                        }
                    }
                    if let Some(r) = chunk.response {
                        if !r.is_empty() {
                            on_chunk(&r);
                        }
                    }
                }
            }
        } else if let Ok(resp) = serde_json::from_str::<ChatCompletionResponse>(trimmed) {
            // Non-streaming fallback
            if let Some(choice) = resp.choices.first() {
                if !choice.message.content.is_empty() {
                    on_chunk(&choice.message.content);
                }
            }
        }
    }

    Ok(())
}

// --------------------------------------------------------------------------
// User Tokens (/api/me/tokens)
// --------------------------------------------------------------------------
#[derive(Deserialize)]
struct TokensWrapper {
    tokens: Vec<UserToken>,
}

pub async fn fetch_user_tokens() -> Result<Vec<UserToken>, String> {
    let url = format!("{}/api/me/tokens", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<TokensWrapper>(res).await?;
    Ok(wrapper.tokens)
}

pub async fn create_user_token(payload: Option<&CreateTokenPayload>) -> Result<CreatedTokenResponse, String> {
    let url = format!("{}/api/me/tokens", get_origin());
    let mut req = apply_auth(client().post(&url));
    if let Some(p) = payload {
        req = req.json(p);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
}

pub async fn update_user_token(token_hash: &str, payload: &UpdateTokenPayload) -> Result<(), String> {
    let url = format!("{}/api/me/tokens/{token_hash}", get_origin());
    let req = apply_auth(client().put(&url)).json(payload);
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Failed to update token: {}", res.status()))
    }
}

pub async fn delete_user_token(token_hash: &str) -> Result<(), String> {
    let url = format!("{}/api/me/tokens/{token_hash}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Failed to delete token: {}", res.status()))
    }
}

// --------------------------------------------------------------------------
// User Logs & Metrics
// --------------------------------------------------------------------------
pub async fn fetch_user_logs(model: Option<&str>, page: Option<i64>, page_size: Option<i64>) -> Result<PaginatedRequests, String> {
    let mut url = format!(
        "{}/api/me/logs?page={}&page_size={}&include_bodies=true",
        get_origin(),
        page.unwrap_or(1),
        page_size.unwrap_or(50)
    );
    if let Some(m) = model {
        if !m.is_empty() {
            url.push_str(&format!("&model={m}"));
        }
    }
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
}

#[derive(Deserialize)]
struct UserMetricsWrapper {
    metrics: Vec<UserMetric>,
}

pub async fn fetch_user_metrics() -> Result<Vec<UserMetric>, String> {
    let url = format!("{}/api/me/metrics", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<UserMetricsWrapper>(res).await?;
    Ok(wrapper.metrics)
}

// --------------------------------------------------------------------------
// Admin Dashboard & Metrics
// --------------------------------------------------------------------------
pub async fn fetch_admin_metrics_summary() -> Result<MetricsSummary, String> {
    let url = format!("{}/api/metrics/summary", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
}

#[derive(Deserialize)]
struct ModelMetricsWrapper {
    model_metrics: Vec<ModelMetric>,
}

pub async fn fetch_admin_model_metrics() -> Result<Vec<ModelMetric>, String> {
    let url = format!("{}/api/metrics/models", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<ModelMetricsWrapper>(res).await?;
    Ok(wrapper.model_metrics)
}

#[derive(Deserialize)]
struct AllUserMetricsWrapper {
    user_metrics: Vec<UserMetric>,
}

pub async fn fetch_admin_user_metrics() -> Result<Vec<UserMetric>, String> {
    let url = format!("{}/api/metrics/users", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<AllUserMetricsWrapper>(res).await?;
    Ok(wrapper.user_metrics)
}

#[derive(Deserialize)]
struct TimeseriesWrapper {
    timeseries: Vec<TimeseriesPoint>,
}

pub async fn fetch_admin_timeseries(points: usize, interval_secs: i64) -> Result<Vec<TimeseriesPoint>, String> {
    let url = format!("{}/api/metrics/timeseries?points={points}&interval_secs={interval_secs}", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<TimeseriesWrapper>(res).await?;
    Ok(wrapper.timeseries)
}

// --------------------------------------------------------------------------
// Admin Models Management (/api/models)
// --------------------------------------------------------------------------
#[derive(Deserialize)]
struct AdminModelsWrapper {
    models: Vec<AdminModelConfig>,
}

pub async fn fetch_admin_models() -> Result<Vec<AdminModelConfig>, String> {
    let url = format!("{}/api/models", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<AdminModelsWrapper>(res).await?;
    Ok(wrapper.models)
}

pub async fn save_admin_model(model: &AdminModelConfig, is_new: bool) -> Result<(), String> {
    let url = if is_new {
        format!("{}/api/models", get_origin())
    } else {
        format!("{}/api/models/{}", get_origin(), model.id)
    };
    let req = if is_new {
        apply_auth(client().post(&url).json(model))
    } else {
        apply_auth(client().put(&url).json(model))
    };
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        let err = res.text().await.unwrap_or_default();
        Err(format!("Save model failed: {err}"))
    }
}

pub async fn delete_admin_model(id: &str) -> Result<(), String> {
    let url = format!("{}/api/models/{id}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete model failed: {}", res.status()))
    }
}

// --------------------------------------------------------------------------
// Admin OpenAI APIs (/api/openai-apis)
// --------------------------------------------------------------------------
#[derive(Deserialize)]
struct OpenAiApisWrapper {
    openai_apis: Vec<OpenAiApiRecord>,
}

pub async fn fetch_admin_openai_apis() -> Result<Vec<OpenAiApiRecord>, String> {
    let url = format!("{}/api/openai-apis", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<OpenAiApisWrapper>(res).await?;
    Ok(wrapper.openai_apis)
}

pub async fn save_admin_openai_api(api: &OpenAiApiRecord, is_new: bool) -> Result<(), String> {
    let url = if is_new {
        format!("{}/api/openai-apis", get_origin())
    } else {
        format!("{}/api/openai-apis/{}", get_origin(), api.id)
    };
    let req = if is_new {
        apply_auth(client().post(&url).json(api))
    } else {
        apply_auth(client().put(&url).json(api))
    };
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        let err = res.text().await.unwrap_or_default();
        Err(format!("Save OpenAI API failed: {err}"))
    }
}

pub async fn delete_admin_openai_api(id: &str) -> Result<(), String> {
    let url = format!("{}/api/openai-apis/{id}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete OpenAI API failed: {}", res.status()))
    }
}

// --------------------------------------------------------------------------
// Admin Backends (/api/backends)
// --------------------------------------------------------------------------
#[derive(Deserialize)]
struct BackendsWrapper {
    backends: Vec<BackendRecord>,
}

pub async fn fetch_admin_backends() -> Result<Vec<BackendRecord>, String> {
    let url = format!("{}/api/backends", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<BackendsWrapper>(res).await?;
    Ok(wrapper.backends)
}

pub async fn save_admin_backend(backend: &BackendRecord, is_new: bool) -> Result<(), String> {
    let url = if is_new {
        format!("{}/api/backends", get_origin())
    } else {
        format!("{}/api/backends/{}", get_origin(), backend.id)
    };
    let req = if is_new {
        apply_auth(client().post(&url).json(backend))
    } else {
        apply_auth(client().put(&url).json(backend))
    };
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        let err = res.text().await.unwrap_or_default();
        Err(format!("Save backend failed: {err}"))
    }
}

pub async fn delete_admin_backend(id: &str) -> Result<(), String> {
    let url = format!("{}/api/backends/{id}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete backend failed: {}", res.status()))
    }
}

#[derive(Deserialize)]
struct BackendModelsWrapper {
    models: Vec<BackendModelRecord>,
}

pub async fn fetch_admin_backend_models(backend_id: &str) -> Result<Vec<BackendModelRecord>, String> {
    let url = format!("{}/api/backends/{backend_id}/models", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<BackendModelsWrapper>(res).await?;
    Ok(wrapper.models)
}

pub async fn save_admin_backend_model(model: &BackendModelRecord, is_new: bool) -> Result<(), String> {
    let url = if is_new {
        format!("{}/api/backends/{}/models", get_origin(), model.backend_id)
    } else {
        format!("{}/api/backends/{}/models/{}", get_origin(), model.backend_id, model.model)
    };
    let req = if is_new {
        apply_auth(client().post(&url).json(model))
    } else {
        apply_auth(client().put(&url).json(model))
    };
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        let err = res.text().await.unwrap_or_default();
        Err(format!("Save backend model failed: {err}"))
    }
}

pub async fn delete_admin_backend_model(backend_id: &str, model: &str) -> Result<(), String> {
    let url = format!("{}/api/backends/{backend_id}/models/{model}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete backend model failed: {}", res.status()))
    }
}

// --------------------------------------------------------------------------
// Admin Users Management (/api/users)
// --------------------------------------------------------------------------
#[derive(Deserialize)]
struct AdminUsersWrapper {
    users: Vec<AdminUserRecord>,
}

pub async fn fetch_admin_users() -> Result<Vec<AdminUserRecord>, String> {
    let url = format!("{}/api/users", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<AdminUsersWrapper>(res).await?;
    Ok(wrapper.users)
}

pub async fn save_admin_user(
    user: &AdminUserRecord,
    rate_limits: Option<&[TokenRateLimitInput]>,
    is_new: bool,
) -> Result<(), String> {
    let url = if is_new {
        format!("{}/api/users", get_origin())
    } else {
        format!("{}/api/users/{}", get_origin(), user.username)
    };
    let payload = serde_json::json!({
        "username": user.username,
        "limit_to_models": user.limit_to_models,
        "log_request_body": user.log_request_body,
        "is_admin": user.is_admin,
        "expires_at": user.expires_at,
        "rate_limits": rate_limits,
    });
    let req = if is_new {
        apply_auth(client().post(&url).json(&payload))
    } else {
        apply_auth(client().put(&url).json(&payload))
    };
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        let err = res.text().await.unwrap_or_default();
        Err(format!("Save user failed: {err}"))
    }
}

pub async fn delete_admin_user(username: &str) -> Result<(), String> {
    let url = format!("{}/api/users/{username}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete user failed: {}", res.status()))
    }
}

pub async fn create_token_for_user(
    username: &str,
    payload: Option<&CreateTokenPayload>,
) -> Result<CreatedTokenResponse, String> {
    let url = format!("{}/api/users/{username}/tokens", get_origin());
    let mut req = apply_auth(client().post(&url));
    if let Some(p) = payload {
        req = req.json(p);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
}

#[allow(dead_code)]
pub async fn update_token_for_user(
    username: &str,
    token_hash: &str,
    payload: &UpdateTokenPayload,
) -> Result<(), String> {
    let url = format!("{}/api/users/{username}/tokens/{token_hash}", get_origin());
    let req = apply_auth(client().put(&url)).json(payload);
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Update token failed: {}", res.status()))
    }
}

#[allow(dead_code)]
pub async fn delete_token_for_user(username: &str, token_hash: &str) -> Result<(), String> {
    let url = format!("{}/api/users/{username}/tokens/{token_hash}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete token failed: {}", res.status()))
    }
}

// --------------------------------------------------------------------------
// Admin Factory GPUs (/api/factory-gpus)
// --------------------------------------------------------------------------
#[derive(Deserialize)]
struct FactoryGpusWrapper {
    factory_gpus: Vec<FactoryGpuRecord>,
}

pub async fn fetch_admin_factory_gpus() -> Result<Vec<FactoryGpuRecord>, String> {
    let url = format!("{}/api/factory-gpus", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<FactoryGpusWrapper>(res).await?;
    Ok(wrapper.factory_gpus)
}

pub async fn save_admin_factory_gpu(gpu: &FactoryGpuRecord) -> Result<(), String> {
    let url = format!("{}/api/factory-gpus", get_origin());
    let req = apply_auth(client().post(&url).json(gpu));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        let err = res.text().await.unwrap_or_default();
        Err(format!("Save factory GPU failed: {err}"))
    }
}

pub async fn delete_admin_factory_gpu(factory_type: &str, id: &str, model: &str) -> Result<(), String> {
    let mut url = format!("{}/api/factory-gpus/{factory_type}/{id}", get_origin());
    if !model.is_empty() {
        url.push_str(&format!("?model={model}"));
    }
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete factory GPU failed: {}", res.status()))
    }
}

// --------------------------------------------------------------------------
// Admin Instances (/api/instances)
// --------------------------------------------------------------------------
#[derive(Deserialize)]
struct InstancesWrapper {
    instances: Vec<InstanceSummary>,
}

pub async fn fetch_admin_instances() -> Result<Vec<InstanceSummary>, String> {
    let url = format!("{}/api/instances", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<InstancesWrapper>(res).await?;
    Ok(wrapper.instances)
}

pub async fn delete_admin_instance(id: &str) -> Result<(), String> {
    let url = format!("{}/api/instances/{id}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete instance failed: {}", res.status()))
    }
}

// --------------------------------------------------------------------------
// Admin Aliases (/api/aliases)
// --------------------------------------------------------------------------
#[derive(Deserialize)]
struct AliasesWrapper {
    aliases: std::collections::HashMap<String, String>,
}

pub async fn fetch_admin_aliases() -> Result<Vec<AliasRecord>, String> {
    let url = format!("{}/api/aliases", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<AliasesWrapper>(res).await?;
    let mut list: Vec<AliasRecord> = wrapper
        .aliases
        .into_iter()
        .map(|(alias, target)| AliasRecord { alias, target })
        .collect();
    list.sort_by(|a, b| a.alias.cmp(&b.alias));
    Ok(list)
}

pub async fn save_admin_alias(alias: &AliasRecord, is_new: bool) -> Result<(), String> {
    let url = if is_new {
        format!("{}/api/aliases", get_origin())
    } else {
        format!("{}/api/aliases/{}", get_origin(), alias.alias)
    };
    let req = if is_new {
        apply_auth(client().post(&url).json(alias))
    } else {
        apply_auth(client().put(&url).json(&serde_json::json!({ "target": alias.target })))
    };
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        let err = res.text().await.unwrap_or_default();
        Err(format!("Save alias failed: {err}"))
    }
}

pub async fn delete_admin_alias(alias: &str) -> Result<(), String> {
    let url = format!("{}/api/aliases/{alias}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete alias failed: {}", res.status()))
    }
}

// --------------------------------------------------------------------------
// Admin Rate Limits (/api/rate-limits) & User Rate Limits (/api/me/rate-limits)
// --------------------------------------------------------------------------
pub async fn fetch_admin_rate_limits(
    user: Option<&str>,
    model: Option<&str>,
) -> Result<Vec<RateLimitRecord>, String> {
    let mut url = format!("{}/api/rate-limits", get_origin());
    let mut params = Vec::new();
    if let Some(u) = user {
        if !u.is_empty() {
            params.push(format!("user={u}"));
        }
    }
    if let Some(m) = model {
        if !m.is_empty() {
            params.push(format!("model={m}"));
        }
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<crate::models::RateLimitListResponse>(res).await?;
    Ok(wrapper.rate_limits)
}

pub async fn fetch_admin_rate_limit_usage(
    user: Option<&str>,
    model: Option<&str>,
) -> Result<Vec<RateLimitUsage>, String> {
    let mut url = format!("{}/api/rate-limits/usage", get_origin());
    let mut params = Vec::new();
    if let Some(u) = user {
        if !u.is_empty() {
            params.push(format!("user={u}"));
        }
    }
    if let Some(m) = model {
        if !m.is_empty() {
            params.push(format!("model={m}"));
        }
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<crate::models::RateLimitUsageListResponse>(res).await?;
    Ok(wrapper.usages)
}

pub async fn fetch_my_rate_limits() -> Result<Vec<RateLimitUsage>, String> {
    let url = format!("{}/api/me/rate-limits", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<crate::models::MyRateLimitsResponse>(res).await?;
    Ok(wrapper.rate_limits)
}

pub async fn fetch_my_models() -> Result<Vec<UserModelStatus>, String> {
    let url = format!("{}/api/me/models", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<crate::models::UserModelsResponse>(res).await?;
    Ok(wrapper.models)
}

pub async fn save_admin_rate_limit(rule: &RateLimitRecord, is_new: bool) -> Result<(), String> {
    let url = if is_new {
        format!("{}/api/rate-limits", get_origin())
    } else {
        format!("{}/api/rate-limits/{}", get_origin(), rule.id)
    };

    let payload = serde_json::json!({
        "user": rule.user,
        "model": rule.model,
        "time_window": rule.time_window,
        "limit_value": rule.limit_value,
        "unit": rule.unit,
    });

    let req = if is_new {
        apply_auth(client().post(&url).json(&payload))
    } else {
        apply_auth(client().put(&url).json(&payload))
    };

    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        let err = res.text().await.unwrap_or_default();
        Err(format!("Save rate limit failed: {err}"))
    }
}

pub async fn delete_admin_rate_limit(id: &str) -> Result<(), String> {
    let url = format!("{}/api/rate-limits/{id}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete rate limit failed: {}", res.status()))
    }
}

// --------------------------------------------------------------------------
// Admin System Logs (/api/logs/requests)
// --------------------------------------------------------------------------
pub async fn fetch_admin_system_logs(
    user: Option<&str>,
    model: Option<&str>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<PaginatedRequests, String> {
    let mut url = format!(
        "{}/api/logs/requests?page={}&page_size={}&include_bodies=true",
        get_origin(),
        page.unwrap_or(1),
        page_size.unwrap_or(50)
    );
    if let Some(u) = user {
        if !u.is_empty() {
            url.push_str(&format!("&user={u}"));
        }
    }
    if let Some(m) = model {
        if !m.is_empty() {
            url.push_str(&format!("&model={m}"));
        }
    }
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
}

#[derive(Deserialize)]
struct CleanResult {
    deleted_count: i64,
}

pub async fn cleanup_admin_system_logs(older_than_secs: i64) -> Result<i64, String> {
    let url = format!("{}/api/logs/requests?older_than={older_than_secs}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let body = handle_json_response::<CleanResult>(res).await?;
    Ok(body.deleted_count)
}

// --------------------------------------------------------------------------
// Admin Groups (/api/groups)
// --------------------------------------------------------------------------
#[derive(Deserialize)]
struct SingleGroupResponse {
    group: Group,
}

#[derive(Deserialize)]
struct SingleRuleResponse {
    rule: GroupOidcRule,
}

pub async fn fetch_admin_groups() -> Result<Vec<GroupSummary>, String> {
    let url = format!("{}/api/groups", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let body = handle_json_response::<GroupsListResponse>(res).await?;
    Ok(body.groups)
}

pub async fn create_admin_group(payload: &CreateGroupPayload) -> Result<Group, String> {
    let url = format!("{}/api/groups", get_origin());
    let req = apply_auth(client().post(&url).json(payload));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let body = handle_json_response::<SingleGroupResponse>(res).await?;
    Ok(body.group)
}

pub async fn fetch_admin_group_detail(id: &str) -> Result<GroupDetail, String> {
    let url = format!("{}/api/groups/{id}", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
}

pub async fn update_admin_group(id: &str, payload: &UpdateGroupPayload) -> Result<Group, String> {
    let url = format!("{}/api/groups/{id}", get_origin());
    let req = apply_auth(client().put(&url).json(payload));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let body = handle_json_response::<SingleGroupResponse>(res).await?;
    Ok(body.group)
}

pub async fn delete_admin_group(id: &str) -> Result<(), String> {
    let url = format!("{}/api/groups/{id}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete group failed: {}", res.status()))
    }
}

pub async fn add_admin_group_member(group_id: &str, username: &str) -> Result<(), String> {
    let url = format!("{}/api/groups/{group_id}/members", get_origin());
    let req = apply_auth(client().post(&url).json(&serde_json::json!({ "username": username })));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        let text = res.text().await.unwrap_or_default();
        Err(format!("Add member failed: {text}"))
    }
}

pub async fn remove_admin_group_member(group_id: &str, username: &str) -> Result<(), String> {
    let url = format!("{}/api/groups/{group_id}/members/{username}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Remove member failed: {}", res.status()))
    }
}

pub async fn create_admin_group_rule(
    group_id: &str,
    payload: &CreateGroupOidcRulePayload,
) -> Result<GroupOidcRule, String> {
    let url = format!("{}/api/groups/{group_id}/rules", get_origin());
    let req = apply_auth(client().post(&url).json(payload));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let body = handle_json_response::<SingleRuleResponse>(res).await?;
    Ok(body.rule)
}

pub async fn delete_admin_group_rule(group_id: &str, rule_id: &str) -> Result<(), String> {
    let url = format!("{}/api/groups/{group_id}/rules/{rule_id}", get_origin());
    let req = apply_auth(client().delete(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete rule failed: {}", res.status()))
    }
}

pub async fn test_cel_rule(payload: &CelTestRequest) -> Result<CelTestResponse, String> {
    let url = format!("{}/api/groups/rules/test", get_origin());
    let req = apply_auth(client().post(&url).json(payload));
    let res = req.send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
}
