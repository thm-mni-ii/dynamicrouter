use crate::auth::AdminUser;
use crate::db::alias::{create_alias, delete_alias, get_alias, get_all_aliases};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize, Serialize)]
pub struct AliasPayload {
    pub alias: String,
    pub target: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateAliasPayload {
    pub target: String,
}

pub fn aliases_routes() -> Router {
    Router::new()
        .route("/", get(list_aliases).post(create_new_alias))
        .route("/{alias}", get(get_single_alias).put(update_existing_alias).delete(remove_alias))
}

async fn list_aliases(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let aliases = get_all_aliases().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch aliases: {e}") })),
        )
    })?;

    Ok(Json(json!({ "aliases": aliases })))
}

async fn get_single_alias(
    _admin: AdminUser,
    Path(alias): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let target = get_alias(&alias).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch alias: {e}") })),
        )
    })?;

    if let Some(t) = target {
        Ok(Json(json!({ "alias": alias, "target": t })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Alias '{alias}' not found") })),
        ))
    }
}

async fn create_new_alias(
    _admin: AdminUser,
    Json(payload): Json<AliasPayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    create_alias(&payload.alias, &payload.target)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Failed to create alias: {e}") })),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "created",
            "alias": payload.alias,
            "target": payload.target,
        })),
    ))
}

async fn update_existing_alias(
    _admin: AdminUser,
    Path(alias): Path<String>,
    Json(payload): Json<UpdateAliasPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    create_alias(&alias, &payload.target)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Failed to update alias: {e}") })),
            )
        })?;

    Ok(Json(json!({
        "status": "updated",
        "alias": alias,
        "target": payload.target,
    })))
}

async fn remove_alias(
    _admin: AdminUser,
    Path(alias): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_alias(&alias).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete alias: {e}") })),
        )
    })?;

    if deleted {
        Ok(Json(json!({ "status": "deleted", "alias": alias })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Alias '{alias}' not found") })),
        ))
    }
}
