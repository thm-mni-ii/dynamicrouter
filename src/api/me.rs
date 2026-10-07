use crate::auth::AuthUser;
use crate::db::alias::get_all_aliases;
use crate::db::backend::get_all_backend_model_configs;
use crate::db::model::{ModelHandlerConfig, get_all_models};
use crate::db::openai_api::get_all_openai_apis;
use crate::db::rate_limit::{
    RateLimitUsageInfo, calculate_usage_for_rule, get_matching_rate_limits,
};
use crate::db::request::{
    RequestFilter, get_batch_models_live_stats, get_requests, get_user_metrics,
};
use crate::db::user::{delete_user_token_for_user, generate_and_add_user_token, get_user_tokens};
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};

pub fn me_routes() -> Router {
    Router::new()
        .route("/", get(get_me))
        .route("/tokens", get(get_tokens).post(create_token))
        .route("/tokens/{token_hash}", delete(delete_token))
        .route("/rate-limits", get(get_my_rate_limits))
        .route("/logs", get(get_logs))
        .route("/metrics", get(get_metrics))
        .route("/models", get(get_my_models))
}

async fn get_me(user: AuthUser) -> Json<serde_json::Value> {
    Json(json!({
        "username": user.username,
        "email": user.email,
        "is_admin": user.is_admin,
        "limit_to_models": user.user.as_ref().and_then(|u| u.limit_to_models.clone()),
        "log_request_body": user.user.as_ref().and_then(|u| u.log_request_body),
        "expires_at": user.user.as_ref().and_then(|u| u.expires_at),
        "is_expired": user.user.as_ref().is_some_and(crate::db::user::User::is_expired),
    }))
}

async fn get_tokens(
    user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let tokens = get_user_tokens(&user.username).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch tokens: {e}") })),
        )
    })?;

    Ok(Json(json!({ "tokens": tokens })))
}

async fn create_token(
    user: AuthUser,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let raw_token = generate_and_add_user_token(&user.username)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to generate token: {e}") })),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "token": raw_token,
            "username": user.username,
            "message": "Token generated. Store this token securely as it will not be displayed again.",
        })),
    ))
}

async fn delete_token(
    user: AuthUser,
    Path(token_hash): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_user_token_for_user(&user.username, &token_hash)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to delete token: {e}") })),
            )
        })?;

    if deleted {
        Ok(Json(json!({ "status": "deleted" })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Token not found" })),
        ))
    }
}

async fn get_my_rate_limits(
    user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let rules = get_matching_rate_limits(&user.username, "*")
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to fetch rate limits: {e}") })),
            )
        })?;

    let now = Utc::now().timestamp();
    let mut usage_list = Vec::new();
    for rule in &rules {
        let model = rule.model.as_deref().unwrap_or("*");
        if let Ok(info) = calculate_usage_for_rule(rule, &user.username, model, now).await {
            usage_list.push(info);
        }
    }

    Ok(Json(json!({ "rate_limits": usage_list })))
}

async fn get_logs(
    user: AuthUser,
    Query(mut filter): Query<RequestFilter>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Force user filter to own username
    filter.user = Some(user.username);
    let paginated = get_requests(&filter).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch logs: {e}") })),
        )
    })?;

    Ok(Json(json!(paginated)))
}

async fn get_metrics(
    user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let metrics = get_user_metrics(Some(&user.username))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to fetch metrics: {e}") })),
            )
        })?;

    Ok(Json(json!({ "metrics": metrics })))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserModelStatusResponse {
    pub id: String,
    pub name: String,
    pub handler_type: String,
    pub status: String,
    pub active_instances: i64,
    pub min_scale: Option<u32>,
    pub max_scale: Option<u32>,
    pub queue_length_for_scale_up: Option<u32>,
    pub timeout: Option<u32>,
    pub total_queue_length: i64,
    pub user_queue_length: i64,
    pub total_output_tps: f64,
    pub user_output_tps: f64,
    pub user_tps_share_pct: f64,
    pub avg_duration_secs: Option<f64>,
    pub ctx_length: Option<u64>,
    pub aliases: Vec<String>,
    pub user_total_requests: i64,
    pub user_total_tokens: i64,
    pub rate_limits: Vec<RateLimitUsageInfo>,
}

struct AccessibleModelMeta {
    id: String,
    name: String,
    handler_type: String,
    min_scale: Option<u32>,
    max_scale: Option<u32>,
    queue_length_for_scale_up: Option<u32>,
    timeout: Option<u32>,
}

async fn get_my_models(
    user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut seen_models = HashSet::new();
    let mut accessible_models: Vec<AccessibleModelMeta> = Vec::new();

    let db_models = get_all_models().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch models: {e}") })),
        )
    })?;

    for model in db_models {
        if !user.is_admin && model.hidden.unwrap_or(model.id == "*") {
            continue;
        }
        if let Some(users) = &model.limit_to_users
            && !users.contains(&user.username)
        {
            continue;
        }
        if let Some(limit_to) = &user.user.as_ref().and_then(|u| u.limit_to_models.as_ref())
            && !limit_to.contains(&model.id)
        {
            continue;
        }

        if seen_models.insert(model.id.clone()) {
            let (handler_type, min_scale, max_scale, q_scale) = match &model.handler {
                ModelHandlerConfig::Spawner(sc) => (
                    "spawner".to_string(),
                    Some(sc.min_scale),
                    Some(sc.max_scale),
                    Some(sc.queue_length_for_scale_up),
                ),
                ModelHandlerConfig::OpenAi(_) => {
                    ("openai".to_string(), None, None, None)
                }
            };
            accessible_models.push(AccessibleModelMeta {
                id: model.id.clone(),
                name: model.name.clone().unwrap_or_else(|| model.id.clone()),
                handler_type,
                min_scale,
                max_scale,
                queue_length_for_scale_up: q_scale,
                timeout: model.timeout,
            });
        }
    }

    let openai_apis = get_all_openai_apis().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch OpenAI APIs: {e}") })),
        )
    })?;

    for api_conf in openai_apis {
        if let Some(api_models) = &api_conf.models {
            for m in api_models {
                let full_id = if let Some(prefix) = &api_conf.prefix {
                    let p = prefix.trim_end_matches('/');
                    format!("{p}/{m}")
                } else {
                    m.clone()
                };

                if let Some(limit_to) = &user.user.as_ref().and_then(|u| u.limit_to_models.as_ref())
                    && !limit_to.contains(&full_id)
                    && !limit_to.contains(m)
                {
                    continue;
                }

                if seen_models.insert(full_id.clone()) {
                    accessible_models.push(AccessibleModelMeta {
                        id: full_id.clone(),
                        name: full_id.clone(),
                        handler_type: "openai".to_string(),
                        min_scale: None,
                        max_scale: None,
                        queue_length_for_scale_up: None,
                        timeout: api_conf.timeout,
                    });
                }
            }
        }
    }

    // Fetch aliases map
    let all_aliases = get_all_aliases().await.unwrap_or_default();
    let mut alias_reverse_map: HashMap<String, Vec<String>> = HashMap::new();
    for (alias, target) in all_aliases {
        alias_reverse_map
            .entry(target)
            .or_default()
            .push(alias);
    }

    // Fetch context lengths map
    let backend_model_configs = get_all_backend_model_configs().await.unwrap_or_default();
    let mut ctx_length_map: HashMap<String, u64> = HashMap::new();
    for bmc in backend_model_configs {
        if let Some(ctx) = bmc.ctx_length {
            ctx_length_map.insert(bmc.model, ctx);
        }
    }

    // Fetch live stats in batch
    let all_model_ids: Vec<String> = accessible_models.iter().map(|m| m.id.clone()).collect();
    let live_stats_map = get_batch_models_live_stats(&all_model_ids, &user.username, 60)
        .await
        .unwrap_or_default();

    let now = Utc::now().timestamp();
    let mut result_models: Vec<UserModelStatusResponse> = Vec::new();

    for model in accessible_models {
        let stats = live_stats_map.get(&model.id).cloned().unwrap_or_default();

        let status = if model.handler_type == "openai" {
            "upstream".to_string()
        } else if stats.active_instances > 0 {
            "running".to_string()
        } else {
            "idle".to_string()
        };

        let aliases = alias_reverse_map.get(&model.id).cloned().unwrap_or_default();
        let ctx_length = ctx_length_map.get(&model.id).copied();

        // Calculate rate limits for this model
        let mut model_rate_limits = Vec::new();
        if let Ok(rules) = get_matching_rate_limits(&user.username, &model.id).await {
            for rule in rules {
                if let Ok(info) = calculate_usage_for_rule(&rule, &user.username, &model.id, now).await {
                    model_rate_limits.push(info);
                }
            }
        }

        result_models.push(UserModelStatusResponse {
            id: model.id,
            name: model.name,
            handler_type: model.handler_type,
            status,
            active_instances: stats.active_instances,
            min_scale: model.min_scale,
            max_scale: model.max_scale,
            queue_length_for_scale_up: model.queue_length_for_scale_up,
            timeout: model.timeout,
            total_queue_length: stats.total_queue_length,
            user_queue_length: stats.user_queue_length,
            total_output_tps: stats.total_output_tps,
            user_output_tps: stats.user_output_tps,
            user_tps_share_pct: stats.user_tps_share_pct,
            avg_duration_secs: stats.avg_duration_secs,
            ctx_length,
            aliases,
            user_total_requests: stats.user_total_requests,
            user_total_tokens: stats.user_total_tokens,
            rate_limits: model_rate_limits,
        });
    }

    // Sort models by name
    result_models.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(Json(json!({ "models": result_models })))
}
