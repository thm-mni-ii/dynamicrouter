use crate::auth::AdminUser;
use crate::db::instance::{delete_instance, get_all_instances, get_instance};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

pub fn instances_routes() -> Router {
    Router::new()
        .route("/", get(list_instances))
        .route("/{id}", get(get_single_instance).delete(remove_instance))
}

async fn list_instances(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let instances = get_all_instances().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch instances: {e}") })),
        )
    })?;

    Ok(Json(json!({ "instances": instances })))
}

async fn get_single_instance(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let instance = get_instance(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch instance: {e}") })),
        )
    })?;

    if let Some(inst) = instance {
        Ok(Json(json!(inst)))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Instance '{id}' not found") })),
        ))
    }
}

async fn remove_instance(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    delete_instance(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete instance: {e}") })),
        )
    })?;

    Ok(Json(json!({ "status": "deleted", "id": id })))
}
