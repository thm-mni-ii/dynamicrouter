use crate::auth::AdminUser;
use crate::db::model::{
    ModelConfig, create_model, delete_model, get_all_models, get_exact_model_config,
};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

pub fn models_routes() -> Router {
    Router::new()
        .route("/", get(list_models).post(create_new_model))
        .route("/{*id}", get(get_model).put(update_model_config).delete(remove_model))
}

async fn list_models(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let models = get_all_models().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch models: {e}") })),
        )
    })?;

    Ok(Json(json!({ "models": models })))
}

async fn get_model(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let model = get_exact_model_config(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch model: {e}") })),
        )
    })?;

    if let Some(m) = model {
        Ok(Json(json!(m)))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Model '{id}' not found") })),
        ))
    }
}

async fn create_new_model(
    _admin: AdminUser,
    Json(payload): Json<ModelConfig>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    create_model(&payload).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to create model: {e}") })),
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

async fn update_model_config(
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(mut payload): Json<ModelConfig>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    payload.id = id.clone();
    create_model(&payload).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to update model: {e}") })),
        )
    })?;

    Ok(Json(json!({
        "status": "updated",
        "id": id,
    })))
}

async fn remove_model(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_model(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete model: {e}") })),
        )
    })?;

    if deleted {
        Ok(Json(json!({ "status": "deleted", "id": id })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Model '{id}' not found") })),
        ))
    }
}
