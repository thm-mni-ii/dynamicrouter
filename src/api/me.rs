use crate::auth::AuthUser;
use crate::db::request::{RequestFilter, get_requests, get_user_metrics};
use crate::db::user::{delete_user_token_for_user, generate_and_add_user_token, get_user_tokens};
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde_json::json;

pub fn me_routes() -> Router {
    Router::new()
        .route("/", get(get_me))
        .route("/tokens", get(get_tokens).post(create_token))
        .route("/tokens/{token_hash}", delete(delete_token))
        .route("/logs", get(get_logs))
        .route("/metrics", get(get_metrics))
}

async fn get_me(user: AuthUser) -> Json<serde_json::Value> {
    Json(json!({
        "username": user.username,
        "email": user.email,
        "is_admin": user.is_admin,
        "limit_to_models": user.user.as_ref().and_then(|u| u.limit_to_models.clone()),
        "log_request_body": user.user.as_ref().and_then(|u| u.log_request_body),
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
