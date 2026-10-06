use crate::auth::AdminUser;
use crate::db::rate_limit::{
    calculate_usage_for_rule, create_rate_limit, delete_rate_limit, get_all_rate_limits,
    get_rate_limit_by_id, update_rate_limit, CreateRateLimitPayload,
    RateLimitUsageInfo, UpdateRateLimitPayload,
};
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct RateLimitQueryFilter {
    pub user: Option<String>,
    pub model: Option<String>,
}

pub fn rate_limits_routes() -> Router {
    Router::new()
        .route("/", get(list_rate_limits).post(create_new_rate_limit))
        .route("/usage", get(get_all_rate_limit_usage))
        .route(
            "/{id}",
            get(get_single_rate_limit)
                .put(update_existing_rate_limit)
                .delete(remove_rate_limit),
        )
}

async fn list_rate_limits(
    _admin: AdminUser,
    Query(filter): Query<RateLimitQueryFilter>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let rules = get_all_rate_limits(filter.user.as_deref(), filter.model.as_deref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to fetch rate limits: {e}") })),
            )
        })?;

    Ok(Json(json!({ "rate_limits": rules })))
}

async fn get_all_rate_limit_usage(
    _admin: AdminUser,
    Query(filter): Query<RateLimitQueryFilter>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let rules = get_all_rate_limits(filter.user.as_deref(), filter.model.as_deref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to fetch rate limits: {e}") })),
            )
        })?;

    let now = Utc::now().timestamp();
    let mut usage_list: Vec<RateLimitUsageInfo> = Vec::new();

    for rule in &rules {
        let username = rule.user.as_deref().unwrap_or("*");
        let model_name = rule.model.as_deref().unwrap_or("*");
        if let Ok(info) = calculate_usage_for_rule(rule, username, model_name, now).await {
            usage_list.push(info);
        }
    }

    Ok(Json(json!({ "usages": usage_list })))
}

async fn get_single_rate_limit(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let rule = get_rate_limit_by_id(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch rate limit: {e}") })),
        )
    })?;

    if let Some(r) = rule {
        Ok(Json(json!(r)))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Rate limit with ID '{id}' not found") })),
        ))
    }
}

async fn create_new_rate_limit(
    _admin: AdminUser,
    Json(payload): Json<CreateRateLimitPayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    if payload.limit_value <= 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "limit_value must be greater than 0" })),
        ));
    }

    let created = create_rate_limit(&payload).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create rate limit: {e}") })),
        )
    })?;

    Ok((StatusCode::CREATED, Json(json!(created))))
}

async fn update_existing_rate_limit(
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(payload): Json<UpdateRateLimitPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if payload.limit_value <= 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "limit_value must be greater than 0" })),
        ));
    }

    let updated = update_rate_limit(&id, &payload).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to update rate limit: {e}") })),
        )
    })?;

    if let Some(r) = updated {
        Ok(Json(json!(r)))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Rate limit with ID '{id}' not found") })),
        ))
    }
}

async fn remove_rate_limit(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_rate_limit(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete rate limit: {e}") })),
        )
    })?;

    if deleted {
        Ok(Json(json!({ "status": "deleted", "id": id })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Rate limit with ID '{id}' not found") })),
        ))
    }
}
