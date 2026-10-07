use crate::models::*;
use reqwest::{Client, Response};
use serde::{de::DeserializeOwned, Deserialize};
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

pub async fn create_user_token() -> Result<CreatedTokenResponse, String> {
    let url = format!("{}/api/me/tokens", get_origin());
    let req = apply_auth(client().post(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
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
pub async fn fetch_user_logs(model: Option<&str>, limit: Option<i64>) -> Result<PaginatedRequests, String> {
    let mut url = format!("{}/api/me/logs?limit={}", get_origin(), limit.unwrap_or(50));
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

pub async fn save_admin_backend_model(model: &BackendModelRecord) -> Result<(), String> {
    let url = format!("{}/api/backends/{}/models", get_origin(), model.backend_id);
    let req = apply_auth(client().post(&url).json(model));
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

pub async fn save_admin_user(user: &AdminUserRecord, is_new: bool) -> Result<(), String> {
    let url = if is_new {
        format!("{}/api/users", get_origin())
    } else {
        format!("{}/api/users/{}", get_origin(), user.username)
    };
    let req = if is_new {
        apply_auth(client().post(&url).json(user))
    } else {
        apply_auth(client().put(&url).json(user))
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

pub async fn create_token_for_user(username: &str) -> Result<CreatedTokenResponse, String> {
    let url = format!("{}/api/users/{username}/tokens", get_origin());
    let req = apply_auth(client().post(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    handle_json_response(res).await
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
    aliases: Vec<AliasRecord>,
}

pub async fn fetch_admin_aliases() -> Result<Vec<AliasRecord>, String> {
    let url = format!("{}/api/aliases", get_origin());
    let req = apply_auth(client().get(&url));
    let res = req.send().await.map_err(|e| e.to_string())?;
    let wrapper = handle_json_response::<AliasesWrapper>(res).await?;
    Ok(wrapper.aliases)
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
    let mut url = format!("{}/api/logs/requests?page={}&page_size={}", 
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
