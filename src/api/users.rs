use crate::auth::AdminUser;
use crate::db::user::{
    create_user, delete_user, delete_user_token, delete_user_token_for_user,
    generate_and_add_user_token, get_all_users, get_user_by_username, get_user_tokens,
};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
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
        .route("/{username}/tokens/{token_hash}", delete(revoke_user_token))
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

    Ok(Json(json!({ "username": username, "tokens": tokens })))
}

async fn create_user_token(
    _admin: AdminUser,
    Path(username): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let raw_token = generate_and_add_user_token(&username)
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
            "message": "Token generated. Store this token securely as it will not be displayed again.",
        })),
    ))
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
