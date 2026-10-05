use crate::auth::AdminUser;
use crate::db::factory::{
    FactoryGpuRecord, create_factory_gpu, delete_factory_gpu, get_all_factory_gpus,
};
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct ListGpusQuery {
    pub factory_type: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteGpuQuery {
    pub model: Option<String>,
}

pub fn factory_gpus_routes() -> Router {
    Router::new()
        .route("/", get(list_gpus).post(create_gpu))
        .route("/{factory_type}/{id}", delete(remove_gpu))
}

async fn list_gpus(
    _admin: AdminUser,
    Query(query): Query<ListGpusQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let gpus = get_all_factory_gpus(query.factory_type.as_deref(), query.model.as_deref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to fetch factory GPUs: {e}") })),
            )
        })?;

    Ok(Json(json!({ "factory_gpus": gpus })))
}

async fn create_gpu(
    _admin: AdminUser,
    Json(payload): Json<FactoryGpuRecord>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let model_opt = if payload.model.is_empty() {
        None
    } else {
        Some(payload.model.as_str())
    };

    create_factory_gpu(
        &payload.factory_type,
        &payload.id,
        payload.memory,
        payload.price,
        payload.max_count,
        model_opt,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to create factory GPU: {e}") })),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "created",
            "factory_type": payload.factory_type,
            "id": payload.id,
            "model": payload.model,
        })),
    ))
}

async fn remove_gpu(
    _admin: AdminUser,
    Path((factory_type, id)): Path<(String, String)>,
    Query(query): Query<DeleteGpuQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_factory_gpu(&factory_type, &id, query.model.as_deref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to delete factory GPU: {e}") })),
            )
        })?;

    if deleted {
        Ok(Json(json!({
            "status": "deleted",
            "factory_type": factory_type,
            "id": id,
            "model": query.model.unwrap_or_default(),
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Factory GPU not found" })),
        ))
    }
}
