use crate::auth::AdminUser;
use crate::db::request::{
    get_metrics_summary, get_model_metrics, get_timeseries_metrics, get_user_metrics,
};
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct TimeseriesQuery {
    pub interval_secs: Option<i64>,
    pub points: Option<usize>,
}

pub fn metrics_routes() -> Router {
    Router::new()
        .route("/summary", get(summary_metrics))
        .route("/models", get(models_metrics))
        .route("/users", get(users_metrics))
        .route("/timeseries", get(timeseries_metrics))
}

async fn summary_metrics(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let summary = get_metrics_summary().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch metrics summary: {e}") })),
        )
    })?;

    Ok(Json(json!(summary)))
}

async fn models_metrics(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let models = get_model_metrics().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch model metrics: {e}") })),
        )
    })?;

    Ok(Json(json!({ "model_metrics": models })))
}

async fn users_metrics(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let users = get_user_metrics(None).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch user metrics: {e}") })),
        )
    })?;

    Ok(Json(json!({ "user_metrics": users })))
}

async fn timeseries_metrics(
    _admin: AdminUser,
    Query(query): Query<TimeseriesQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let interval = query.interval_secs.unwrap_or(300); // 5 mins default
    let points = query.points.unwrap_or(288); // 24 hours of 5-min buckets

    let data = get_timeseries_metrics(interval, points)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to fetch timeseries metrics: {e}") })),
            )
        })?;

    Ok(Json(json!({ "timeseries": data })))
}

use std::fmt::Write;

pub async fn prometheus_metrics() -> impl IntoResponse {
    let Ok(summary) = get_metrics_summary().await else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Error computing metrics").into_response();
    };

    let model_metrics = get_model_metrics().await.unwrap_or_default();

    let mut output = String::new();
    output.push_str("# HELP dynamicrouter_requests_total Total number of requests processed\n");
    output.push_str("# TYPE dynamicrouter_requests_total counter\n");
    let _ = writeln!(output, "dynamicrouter_requests_total {}\n", summary.total_requests);

    output.push_str("# HELP dynamicrouter_active_requests Number of currently pending requests\n");
    output.push_str("# TYPE dynamicrouter_active_requests gauge\n");
    let _ = writeln!(output, "dynamicrouter_active_requests {}\n", summary.pending_requests);

    output.push_str("# HELP dynamicrouter_instances_active Number of active model instances\n");
    output.push_str("# TYPE dynamicrouter_instances_active gauge\n");
    let _ = writeln!(output, "dynamicrouter_instances_active {}\n", summary.active_instances);

    if let Some(avg) = summary.avg_duration_secs {
        output.push_str("# HELP dynamicrouter_request_duration_avg_seconds Average duration of completed requests\n");
        output.push_str("# TYPE dynamicrouter_request_duration_avg_seconds gauge\n");
        let _ = writeln!(output, "dynamicrouter_request_duration_avg_seconds {avg:.4}\n");
    }

    if let Some(p50) = summary.p50_duration_secs {
        output.push_str("# HELP dynamicrouter_request_duration_p50_seconds Median duration of completed requests\n");
        output.push_str("# TYPE dynamicrouter_request_duration_p50_seconds gauge\n");
        let _ = writeln!(output, "dynamicrouter_request_duration_p50_seconds {p50:.4}\n");
    }

    if let Some(p99) = summary.p99_duration_secs {
        output.push_str("# HELP dynamicrouter_request_duration_p99_seconds 99th percentile duration of completed requests\n");
        output.push_str("# TYPE dynamicrouter_request_duration_p99_seconds gauge\n");
        let _ = writeln!(output, "dynamicrouter_request_duration_p99_seconds {p99:.4}\n");
    }

    output.push_str("# HELP dynamicrouter_model_requests_total Number of requests per model\n");
    output.push_str("# TYPE dynamicrouter_model_requests_total counter\n");
    for mm in &model_metrics {
        let _ = writeln!(
            output,
            "dynamicrouter_model_requests_total{{model=\"{}\"}} {}",
            mm.model, mm.total_requests
        );
    }
    output.push('\n');

    output.push_str("# HELP dynamicrouter_model_instances Number of active instances per model\n");
    output.push_str("# TYPE dynamicrouter_model_instances gauge\n");
    for mm in &model_metrics {
        let _ = writeln!(
            output,
            "dynamicrouter_model_instances{{model=\"{}\"}} {}",
            mm.model, mm.active_instances
        );
    }
    output.push('\n');

    (
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        output,
    )
        .into_response()
}
