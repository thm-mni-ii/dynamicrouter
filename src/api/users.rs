use crate::auth::AdminUser;
use crate::db::rate_limit::calculate_usage_for_rule;
use crate::db::user::{
    CreateTokenInput, UpdateTokenInput, create_user, delete_user, delete_user_token,
    delete_user_token_for_user, generate_and_add_user_token, get_all_users, get_user_by_username,
    get_user_tokens, update_user_token_for_user,
};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{delete, get, put};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateUserPayload {
    pub username: String,
    pub limit_to_models: Option<HashSet<String>>,
    pub log_request_body: Option<bool>,
    pub is_admin: Option<bool>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateUserPayload {
    pub limit_to_models: Option<HashSet<String>>,
    pub log_request_body: Option<bool>,
    pub is_admin: Option<bool>,
    pub expires_at: Option<i64>,
}

pub fn users_routes() -> Router {
    Router::new()
        .route("/", get(list_users).post(create_new_user))
        .route("/tokens/{token_hash}", delete(delete_any_token))
        .route("/{username}", get(get_single_user).put(update_existing_user).delete(remove_user))
        .route("/{username}/tokens", get(list_user_tokens).post(create_user_token))
        .route("/{username}/tokens/{token_hash}", put(update_user_token).delete(revoke_user_token))
}

async fn list_users(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let users = get_all_users().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch users: {e}") })),
        )
    })?;

    Ok(Json(json!({ "users": users })))
}

async fn get_single_user(
    _admin: AdminUser,
    Path(username): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user = get_user_by_username(&username).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch user: {e}") })),
        )
    })?;

    if let Some(u) = user {
        Ok(Json(json!(u)))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("User '{username}' not found") })),
        ))
    }
}

async fn create_new_user(
    _admin: AdminUser,
    Json(payload): Json<CreateUserPayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    create_user(
        &payload.username,
        payload.limit_to_models.as_ref(),
        payload.log_request_body,
        payload.is_admin.unwrap_or(false),
        payload.expires_at,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to create user: {e}") })),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "created",
            "username": payload.username,
        })),
    ))
}

async fn update_existing_user(
    _admin: AdminUser,
    Path(username): Path<String>,
    Json(payload): Json<UpdateUserPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let existing = get_user_by_username(&username).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch user: {e}") })),
        )
    })?;

    if existing.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("User '{username}' not found") })),
        ));
    }

    let is_admin = payload
        .is_admin
        .unwrap_or_else(|| existing.as_ref().is_some_and(|u| u.is_admin));

    create_user(
        &username,
        payload.limit_to_models.as_ref(),
        payload.log_request_body,
        is_admin,
        payload.expires_at,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to update user: {e}") })),
        )
    })?;

    Ok(Json(json!({
        "status": "updated",
        "username": username,
    })))
}

async fn remove_user(
    _admin: AdminUser,
    Path(username): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_user(&username).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete user: {e}") })),
        )
    })?;

    if deleted {
        Ok(Json(json!({ "status": "deleted", "username": username })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("User '{username}' not found") })),
        ))
    }
}

async fn list_user_tokens(
    _admin: AdminUser,
    Path(username): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let tokens = get_user_tokens(&username).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch user tokens: {e}") })),
        )
    })?;

    let now = Utc::now().timestamp();
    let mut enriched_tokens = Vec::new();

    for token in tokens {
        let mut enriched_rules = Vec::new();
        for rule in &token.rate_limits {
            let model = rule.model.as_deref().unwrap_or("*");
            if let Ok(info) = calculate_usage_for_rule(rule, &username, model, now).await {
                enriched_rules.push(info);
            }
        }

        enriched_tokens.push(json!({
            "token_hash": token.token_hash,
            "username": token.username,
            "name": token.name,
            "token_prefix": token.token_prefix,
            "token_suffix": token.token_suffix,
            "preview": token.preview_display(),
            "limit_to_models": token.limit_to_models,
            "expires_at": token.expires_at,
            "is_expired": token.is_expired(),
            "allowed_ips": token.allowed_ips,
            "rate_limits": enriched_rules,
            "created_at": token.created_at,
        }));
    }

    Ok(Json(json!({ "username": username, "tokens": enriched_tokens })))
}

async fn create_user_token(
    _admin: AdminUser,
    Path(username): Path<String>,
    payload: Option<Json<CreateTokenInput>>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let input = payload.map(|p| p.0);
    let (raw_token, user_token) = generate_and_add_user_token(&username, input)
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
            "username": username,
            "token_info": {
                "token_hash": user_token.token_hash,
                "name": user_token.name,
                "token_prefix": user_token.token_prefix,
                "token_suffix": user_token.token_suffix,
                "preview": user_token.preview_display(),
                "limit_to_models": user_token.limit_to_models,
                "expires_at": user_token.expires_at,
                "allowed_ips": user_token.allowed_ips,
                "rate_limits": user_token.rate_limits,
                "created_at": user_token.created_at,
            },
            "message": "Token generated. Store this token securely as it will not be displayed again.",
        })),
    ))
}

async fn update_user_token(
    _admin: AdminUser,
    Path((username, token_hash)): Path<(String, String)>,
    Json(payload): Json<UpdateTokenInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let updated = update_user_token_for_user(&username, &token_hash, payload)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to update token: {e}") })),
            )
        })?;

    if let Some(tok) = updated {
        Ok(Json(json!({
            "status": "updated",
            "token_info": {
                "token_hash": tok.token_hash,
                "name": tok.name,
                "token_prefix": tok.token_prefix,
                "token_suffix": tok.token_suffix,
                "preview": tok.preview_display(),
                "limit_to_models": tok.limit_to_models,
                "expires_at": tok.expires_at,
                "allowed_ips": tok.allowed_ips,
                "rate_limits": tok.rate_limits,
                "created_at": tok.created_at,
            }
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Token not found" })),
        ))
    }
}

async fn delete_any_token(
    _admin: AdminUser,
    Path(token_hash): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_user_token(&token_hash).await.map_err(|e| {
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

async fn revoke_user_token(
    _admin: AdminUser,
    Path((username, token_hash)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_user_token_for_user(&username, &token_hash)
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
