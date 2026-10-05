use crate::auth::AdminUser;
use crate::db::openai_api::{
    OpenAiApiRecord, create_openai_api, delete_openai_api, get_all_openai_apis, get_openai_api,
};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use secrecy::ExposeSecret;
use serde_json::json;

pub fn openai_apis_routes() -> Router {
    Router::new()
        .route("/", get(list_openai_apis).post(create_new_api))
        .route("/{id}", get(get_single_api).put(update_api).delete(remove_api))
}

async fn list_openai_apis(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let apis = get_all_openai_apis().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch OpenAI APIs: {e}") })),
        )
    })?;

    Ok(Json(json!({ "openai_apis": apis })))
}

async fn get_single_api(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let api = get_openai_api(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch OpenAI API: {e}") })),
        )
    })?;

    if let Some(a) = api {
        Ok(Json(json!(a)))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("OpenAI API '{id}' not found") })),
        ))
    }
}

async fn create_new_api(
    _admin: AdminUser,
    Json(payload): Json<OpenAiApiRecord>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    create_openai_api(
        &payload.id,
        payload.url.as_str(),
        payload.token.as_ref().map(|t| t.expose_secret().as_str()),
        payload.prefix.as_deref(),
        payload.models.as_ref(),
        payload.timeout,
        payload.headers.as_ref(),
    )
    .await
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to create OpenAI API: {e}") })),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "created",
            "id": payload.id,
        })),
    ))
}

async fn update_api(
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(payload): Json<OpenAiApiRecord>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    create_openai_api(
        &id,
        payload.url.as_str(),
        payload.token.as_ref().map(|t| t.expose_secret().as_str()),
        payload.prefix.as_deref(),
        payload.models.as_ref(),
        payload.timeout,
        payload.headers.as_ref(),
    )
    .await
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to update OpenAI API: {e}") })),
        )
    })?;

    Ok(Json(json!({
        "status": "updated",
        "id": id,
    })))
}

async fn remove_api(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_openai_api(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete OpenAI API: {e}") })),
        )
    })?;

    if deleted {
        Ok(Json(json!({ "status": "deleted", "id": id })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("OpenAI API '{id}' not found") })),
        ))
    }
}
