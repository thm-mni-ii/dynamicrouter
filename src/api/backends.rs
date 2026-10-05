use crate::auth::AdminUser;
use crate::db::backend::{
    BackendModelRecord, BackendRecord, create_backend, create_backend_model_config, delete_backend,
    delete_backend_model_config, get_all_backend_model_configs, get_all_backends, get_backend,
    get_backend_model_config, get_backend_models,
};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateBackendPayload {
    pub id: String,
    pub backend_type: String,
    pub image: String,
    pub use_for: Option<crate::db::backend::BackendUseFor>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BackendModelConfigPayload {
    pub model: String,
    pub arguments: Option<Vec<String>>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub ctx_length: Option<u64>,
}

pub fn backends_routes() -> Router {
    Router::new()
        .route("/", get(list_backends).post(create_new_backend))
        .route("/{id}", get(get_single_backend).put(update_existing_backend).delete(remove_backend))
        .route("/{id}/models", get(list_backend_models).post(create_new_backend_model_config))
        .route("/{id}/models/{*model}", get(get_single_backend_model).put(update_backend_model).delete(remove_backend_model))
}

pub fn backend_model_configs_global_routes() -> Router {
    Router::new().route("/", get(list_all_backend_model_configs))
}

async fn list_backends(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let backends = get_all_backends().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch backends: {e}") })),
        )
    })?;

    Ok(Json(json!({ "backends": backends })))
}

async fn get_single_backend(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let backend = get_backend(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch backend: {e}") })),
        )
    })?;

    if let Some(b) = backend {
        Ok(Json(json!(b)))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Backend '{id}' not found") })),
        ))
    }
}

async fn create_new_backend(
    _admin: AdminUser,
    Json(payload): Json<CreateBackendPayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let use_for = payload.use_for.and_then(|u| u.0);
    create_backend(&payload.id, &payload.backend_type, &payload.image, use_for.as_ref())
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Failed to create backend: {e}") })),
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

async fn update_existing_backend(
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(payload): Json<BackendRecord>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let use_for = payload.use_for.0;
    create_backend(&id, &payload.backend_type, &payload.image, use_for.as_ref())
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Failed to update backend: {e}") })),
            )
        })?;

    Ok(Json(json!({
        "status": "updated",
        "id": id,
    })))
}

async fn remove_backend(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_backend(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete backend: {e}") })),
        )
    })?;

    if deleted {
        Ok(Json(json!({ "status": "deleted", "id": id })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Backend '{id}' not found") })),
        ))
    }
}

async fn list_all_backend_model_configs(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let configs = get_all_backend_model_configs().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch backend model configs: {e}") })),
        )
    })?;

    Ok(Json(json!({ "backend_model_configs": configs })))
}

async fn list_backend_models(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let models = get_backend_models(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch backend models: {e}") })),
        )
    })?;

    Ok(Json(json!({ "backend_id": id, "models": models })))
}

async fn get_single_backend_model(
    _admin: AdminUser,
    Path((id, model)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let config = get_backend_model_config(&id, &model).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch backend model config: {e}") })),
        )
    })?;

    if let Some(c) = config {
        Ok(Json(json!(c)))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": format!("Backend model config for backend '{id}' and model '{model}' not found")
            })),
        ))
    }
}

async fn create_new_backend_model_config(
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(payload): Json<BackendModelConfigPayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let empty_args = Vec::new();
    let empty_env = HashMap::new();
    let args = payload.arguments.as_ref().unwrap_or(&empty_args);
    let env = payload.environment_variables.as_ref().unwrap_or(&empty_env);

    create_backend_model_config(&id, &payload.model, args, env, payload.ctx_length)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Failed to create backend model config: {e}") })),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "created",
            "backend_id": id,
            "model": payload.model,
        })),
    ))
}

async fn update_backend_model(
    _admin: AdminUser,
    Path((id, model)): Path<(String, String)>,
    Json(payload): Json<BackendModelRecord>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    create_backend_model_config(
        &id,
        &model,
        &payload.arguments,
        &payload.environment_variables,
        payload.ctx_length,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to update backend model config: {e}") })),
        )
    })?;

    Ok(Json(json!({
        "status": "updated",
        "backend_id": id,
        "model": model,
    })))
}

async fn remove_backend_model(
    _admin: AdminUser,
    Path((id, model)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_backend_model_config(&id, &model).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete backend model config: {e}") })),
        )
    })?;

    if deleted {
        Ok(Json(json!({
            "status": "deleted",
            "backend_id": id,
            "model": model,
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": format!("Backend model config for backend '{id}' and model '{model}' not found")
            })),
        ))
    }
}
