use crate::auth::AdminUser;
use crate::db::request::{
    RequestFilter, delete_old_requests, get_request_by_id, get_requests,
};
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct CleanupLogsQuery {
    pub older_than: i64,
}

pub fn logs_routes() -> Router {
    Router::new()
        .route("/requests", get(list_request_logs).delete(cleanup_request_logs))
        .route("/requests/{id}", get(get_single_request_log))
}

async fn list_request_logs(
    _admin: AdminUser,
    Query(filter): Query<RequestFilter>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let paginated = get_requests(&filter).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch request logs: {e}") })),
        )
    })?;

    Ok(Json(json!(paginated)))
}

async fn get_single_request_log(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let request = get_request_by_id(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch request log: {e}") })),
        )
    })?;

    if let Some(req) = request {
        // Try parsing body strings into JSON objects if valid JSON for prettier inspection
        let parsed_req_body: Option<serde_json::Value> = req
            .request_body
            .as_deref()
            .and_then(|b| serde_json::from_str(b).ok());
        let parsed_res_body: Option<serde_json::Value> = req
            .response_body
            .as_deref()
            .and_then(|b| serde_json::from_str(b).ok());

        Ok(Json(json!({
            "id": req.id,
            "model": req.model,
            "enqueue": req.enqueue,
            "done": req.done,
            "duration_secs": req.done.map(|d| d - req.enqueue),
            "instance": req.instance,
            "user": req.user,
            "request_body": parsed_req_body.unwrap_or_else(|| json!(req.request_body)),
            "response_body": parsed_res_body.unwrap_or_else(|| json!(req.response_body)),
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Request '{id}' not found") })),
        ))
    }
}

async fn cleanup_request_logs(
    _admin: AdminUser,
    Query(query): Query<CleanupLogsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted_count = delete_old_requests(query.older_than)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to delete old request logs: {e}") })),
            )
        })?;

    Ok(Json(json!({
        "status": "cleaned",
        "deleted_count": deleted_count,
        "older_than": query.older_than,
    })))
}
