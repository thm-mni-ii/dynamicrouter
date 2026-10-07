use crate::db::connection::get_executor;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct Request {
    pub id: String,
    pub model: String,
    pub enqueue: i64,
    pub done: Option<i64>,
    pub instance: String,
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<String>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RequestFilter {
    pub model: Option<String>,
    pub user: Option<String>,
    pub instance: Option<String>,
    pub status: Option<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub include_bodies: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestLog {
    pub id: String,
    pub model: String,
    pub enqueue: i64,
    pub done: Option<i64>,
    pub user: Option<String>,
    pub instance: Option<String>,
    pub request_body: Option<serde_json::Value>,
    pub response_body: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaginatedRequests {
    pub requests: Vec<RequestLog>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub total_requests: i64,
    pub pending_requests: i64,
    pub completed_requests: i64,
    pub active_instances: i64,
    pub avg_duration_secs: Option<f64>,
    pub p50_duration_secs: Option<f64>,
    pub p90_duration_secs: Option<f64>,
    pub p95_duration_secs: Option<f64>,
    pub p99_duration_secs: Option<f64>,
    pub requests_last_1m: i64,
    pub requests_last_5m: i64,
    pub requests_last_1h: i64,
    pub requests_last_24h: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub model: String,
    pub total_requests: i64,
    pub pending_requests: i64,
    pub completed_requests: i64,
    pub avg_duration_secs: Option<f64>,
    pub active_instances: i64,
    pub last_request_enqueue: Option<i64>,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMetrics {
    pub user: String,
    pub total_requests: i64,
    pub pending_requests: i64,
    pub completed_requests: i64,
    pub avg_duration_secs: Option<f64>,
    pub last_request_enqueue: Option<i64>,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeseriesPoint {
    pub timestamp: i64,
    pub request_count: i64,
    pub avg_duration_secs: Option<f64>,
    pub total_tokens: i64,
}

pub async fn count_pending_requests_for(model: &str) -> Result<i64, anyhow::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT id) FROM request WHERE model = ? AND done IS NULL",
    )
    .bind(model)
    .fetch_one(get_executor())
    .await?;
    Ok(count)
}

pub async fn latest_for_each_model() -> Result<Vec<Request>, anyhow::Error> {
    Ok(sqlx::query_as::<_, Request>(
        "SELECT id, model, MAX(enqueue) AS enqueue, done, instance, user, request_body, response_body, prompt_tokens, completion_tokens, total_tokens FROM request GROUP BY model"
    )
    .fetch_all(get_executor())
    .await?)
}

pub async fn create_request(
    model: &str,
    instance: &str,
    user: &str,
    request_body: Option<&serde_json::Value>,
    prompt_tokens: Option<i64>,
) -> Result<String, anyhow::Error> {
    let id = Uuid::new_v4().to_string();
    let body_str = request_body.map(serde_json::to_string).transpose()?;
    let p_tokens = prompt_tokens.unwrap_or(0);
    sqlx::query(
        "INSERT INTO request (id, model, enqueue, instance, user, request_body, prompt_tokens, completion_tokens, total_tokens) VALUES (?, ?, unixepoch(), ?, ?, ?, ?, 0, ?)",
    )
    .bind(&id)
    .bind(model)
    .bind(instance)
    .bind(user)
    .bind(body_str)
    .bind(p_tokens)
    .bind(p_tokens)
    .execute(get_executor())
    .await?;
    Ok(id)
}

pub async fn request_done(id: &str) -> Result<(), anyhow::Error> {
    sqlx::query("UPDATE request SET done = unixepoch() WHERE id = ?")
        .bind(id)
        .execute(get_executor())
        .await?;
    Ok(())
}

pub async fn save_response_body(id: &str, body: &str) -> Result<(), anyhow::Error> {
    sqlx::query("UPDATE request SET response_body = ? WHERE id = ?")
        .bind(body)
        .bind(id)
        .execute(get_executor())
        .await?;
    Ok(())
}

pub async fn save_response_body_and_tokens(
    id: &str,
    body: &str,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
) -> Result<(), anyhow::Error> {
    sqlx::query(
        "UPDATE request SET response_body = ?, prompt_tokens = ?, completion_tokens = ?, total_tokens = ? WHERE id = ?",
    )
    .bind(body)
    .bind(prompt_tokens)
    .bind(completion_tokens)
    .bind(total_tokens)
    .bind(id)
    .execute(get_executor())
    .await?;
    Ok(())
}

pub async fn update_request_tokens(
    id: &str,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
) -> Result<(), anyhow::Error> {
    sqlx::query(
        "UPDATE request SET prompt_tokens = ?, completion_tokens = ?, total_tokens = ? WHERE id = ?",
    )
    .bind(prompt_tokens)
    .bind(completion_tokens)
    .bind(total_tokens)
    .bind(id)
    .execute(get_executor())
    .await?;
    Ok(())
}

pub async fn get_request_by_id(id: &str) -> Result<Option<Request>, anyhow::Error> {
    let req = sqlx::query_as::<_, Request>(
        "SELECT id, model, enqueue, done, instance, user, request_body, response_body, prompt_tokens, completion_tokens, total_tokens FROM request WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(get_executor())
    .await?;

    Ok(req)
}

pub async fn get_requests(filter: &RequestFilter) -> Result<PaginatedRequests, anyhow::Error> {
    let page = filter.page.unwrap_or(1).max(1);
    let page_size = filter.page_size.unwrap_or(50).clamp(1, 1000);
    let offset = (page - 1) * page_size;
    let include_bodies = filter.include_bodies.unwrap_or(false);

    let mut where_clauses = Vec::new();
    let mut count_query = "SELECT COUNT(*) FROM request".to_string();

    if let Some(ref m) = filter.model {
        where_clauses.push(format!("model = '{}'", m.replace('\'', "''")));
    }
    if let Some(ref u) = filter.user {
        where_clauses.push(format!("user = '{}'", u.replace('\'', "''")));
    }
    if let Some(ref inst) = filter.instance {
        where_clauses.push(format!("instance = '{}'", inst.replace('\'', "''")));
    }
    if let Some(ref st) = filter.status {
        match st.as_str() {
            "pending" => where_clauses.push("done IS NULL".to_string()),
            "completed" => where_clauses.push("done IS NOT NULL".to_string()),
            _ => {}
        }
    }
    if let Some(from) = filter.from {
        where_clauses.push(format!("enqueue >= {from}"));
    }
    if let Some(to) = filter.to {
        where_clauses.push(format!("enqueue <= {to}"));
    }

    let where_str = if where_clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_clauses.join(" AND "))
    };

    count_query.push_str(&where_str);
    let total: i64 = sqlx::query_scalar(&count_query)
        .fetch_one(get_executor())
        .await?;

    let select_columns = if include_bodies {
        "id, model, enqueue, done, instance, user, request_body, response_body, prompt_tokens, completion_tokens, total_tokens"
    } else {
        "id, model, enqueue, done, instance, user, NULL AS request_body, NULL AS response_body, prompt_tokens, completion_tokens, total_tokens"
    };

    let data_query = format!(
        "SELECT {select_columns} FROM request{where_str} ORDER BY enqueue DESC LIMIT {page_size} OFFSET {offset}"
    );

    let items = sqlx::query_as::<_, Request>(&data_query)
        .fetch_all(get_executor())
        .await?;

    let requests = items
        .into_iter()
        .map(|r| {
            let request_body = r.request_body.and_then(|text| {
                if text.trim().is_empty() {
                    None
                } else {
                    Some(serde_json::from_str(&text).unwrap_or_else(|_| serde_json::Value::String(text)))
                }
            });
            let response_body = r.response_body.and_then(|text| {
                if text.trim().is_empty() {
                    None
                } else {
                    Some(serde_json::from_str(&text).unwrap_or_else(|_| serde_json::Value::String(text)))
                }
            });
            RequestLog {
                id: r.id,
                model: r.model,
                enqueue: r.enqueue,
                done: r.done,
                user: r.user,
                instance: Some(r.instance),
                request_body,
                response_body,
            }
        })
        .collect();

    Ok(PaginatedRequests {
        requests,
        total,
        page,
        page_size,
    })
}

pub async fn delete_old_requests(older_than: i64) -> Result<u64, anyhow::Error> {
    let res = sqlx::query("DELETE FROM request WHERE enqueue < ?")
        .bind(older_than)
        .execute(get_executor())
        .await?;
    Ok(res.rows_affected())
}

#[allow(clippy::similar_names)]
pub async fn get_metrics_summary() -> Result<MetricsSummary, anyhow::Error> {
    let now = Utc::now().timestamp();

    let total_requests: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM request")
        .fetch_one(get_executor())
        .await
        .unwrap_or(0);

    let pending_requests: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM request WHERE done IS NULL")
            .fetch_one(get_executor())
            .await
            .unwrap_or(0);

    let completed_requests: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM request WHERE done IS NOT NULL")
            .fetch_one(get_executor())
            .await
            .unwrap_or(0);

    let active_instances: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instance")
        .fetch_one(get_executor())
        .await
        .unwrap_or(0);

    let avg_duration_secs: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(done - enqueue) FROM request WHERE done IS NOT NULL AND done >= enqueue",
    )
    .fetch_optional(get_executor())
    .await
    .unwrap_or(None);

    let durations: Vec<i64> = sqlx::query_scalar(
        "SELECT (done - enqueue) AS dur FROM request WHERE done IS NOT NULL AND done >= enqueue ORDER BY dur ASC",
    )
    .fetch_all(get_executor())
    .await
    .unwrap_or_default();

    let calc_percentile = |pct: f64| -> Option<f64> {
        if durations.is_empty() {
            return None;
        }
        let rank = (pct / 100.0 * (durations.len() as f64 - 1.0)).round() as usize;
        let idx = rank.min(durations.len() - 1);
        Some(durations[idx] as f64)
    };

    let p50_duration_secs = calc_percentile(50.0);
    let p90_duration_secs = calc_percentile(90.0);
    let p95_duration_secs = calc_percentile(95.0);
    let p99_duration_secs = calc_percentile(99.0);

    let requests_last_1m: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM request WHERE enqueue >= ?")
            .bind(now - 60)
            .fetch_one(get_executor())
            .await
            .unwrap_or(0);

    let requests_last_5m: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM request WHERE enqueue >= ?")
            .bind(now - 300)
            .fetch_one(get_executor())
            .await
            .unwrap_or(0);

    let requests_last_1h: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM request WHERE enqueue >= ?")
            .bind(now - 3600)
            .fetch_one(get_executor())
            .await
            .unwrap_or(0);

    let requests_last_24h: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM request WHERE enqueue >= ?")
            .bind(now - 86400)
            .fetch_one(get_executor())
            .await
            .unwrap_or(0);

    let total_prompt_tokens: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(prompt_tokens), 0) FROM request")
            .fetch_one(get_executor())
            .await
            .unwrap_or(0);

    let total_completion_tokens: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(completion_tokens), 0) FROM request")
            .fetch_one(get_executor())
            .await
            .unwrap_or(0);

    let total_tokens: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(total_tokens), 0) FROM request")
            .fetch_one(get_executor())
            .await
            .unwrap_or(0);

    Ok(MetricsSummary {
        total_requests,
        pending_requests,
        completed_requests,
        active_instances,
        avg_duration_secs,
        p50_duration_secs,
        p90_duration_secs,
        p95_duration_secs,
        p99_duration_secs,
        requests_last_1m,
        requests_last_5m,
        requests_last_1h,
        requests_last_24h,
        total_prompt_tokens,
        total_completion_tokens,
        total_tokens,
    })
}

#[derive(sqlx::FromRow)]
struct ModelMetricsRow {
    model: String,
    total_requests: i64,
    pending_requests: i64,
    completed_requests: i64,
    avg_duration: Option<f64>,
    last_enqueue: Option<i64>,
    total_tokens: Option<i64>,
}

pub async fn get_model_metrics() -> Result<Vec<ModelMetrics>, anyhow::Error> {
    let rows = sqlx::query_as::<_, ModelMetricsRow>(
        r"
        SELECT
            model,
            COUNT(*) AS total_requests,
            SUM(CASE WHEN done IS NULL THEN 1 ELSE 0 END) AS pending_requests,
            SUM(CASE WHEN done IS NOT NULL THEN 1 ELSE 0 END) AS completed_requests,
            AVG(CASE WHEN done IS NOT NULL AND done >= enqueue THEN (done - enqueue) ELSE NULL END) AS avg_duration,
            MAX(enqueue) AS last_enqueue,
            SUM(total_tokens) AS total_tokens
        FROM request
        GROUP BY model
        ORDER BY total_requests DESC
        ",
    )
    .fetch_all(get_executor())
    .await?;

    let mut result = Vec::new();
    for row in rows {
        let active_instances: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM instance WHERE model = ?")
                .bind(&row.model)
                .fetch_one(get_executor())
                .await
                .unwrap_or(0);

        result.push(ModelMetrics {
            model: row.model,
            total_requests: row.total_requests,
            pending_requests: row.pending_requests,
            completed_requests: row.completed_requests,
            avg_duration_secs: row.avg_duration,
            active_instances,
            last_request_enqueue: row.last_enqueue,
            total_tokens: row.total_tokens.unwrap_or(0),
        });
    }

    Ok(result)
}

#[derive(sqlx::FromRow)]
struct UserMetricsRow {
    user: Option<String>,
    total_requests: i64,
    pending_requests: i64,
    completed_requests: i64,
    avg_duration: Option<f64>,
    last_enqueue: Option<i64>,
    total_tokens: Option<i64>,
}

pub async fn get_user_metrics(username: Option<&str>) -> Result<Vec<UserMetrics>, anyhow::Error> {
    let rows = if let Some(u) = username {
        sqlx::query_as::<_, UserMetricsRow>(
            r"
            SELECT
                user,
                COUNT(*) AS total_requests,
                SUM(CASE WHEN done IS NULL THEN 1 ELSE 0 END) AS pending_requests,
                SUM(CASE WHEN done IS NOT NULL THEN 1 ELSE 0 END) AS completed_requests,
                AVG(CASE WHEN done IS NOT NULL AND done >= enqueue THEN (done - enqueue) ELSE NULL END) AS avg_duration,
                MAX(enqueue) AS last_enqueue,
                SUM(total_tokens) AS total_tokens
            FROM request
            WHERE user = ?
            GROUP BY user
            ",
        )
        .bind(u)
        .fetch_all(get_executor())
        .await?
    } else {
        sqlx::query_as::<_, UserMetricsRow>(
            r"
            SELECT
                user,
                COUNT(*) AS total_requests,
                SUM(CASE WHEN done IS NULL THEN 1 ELSE 0 END) AS pending_requests,
                SUM(CASE WHEN done IS NOT NULL THEN 1 ELSE 0 END) AS completed_requests,
                AVG(CASE WHEN done IS NOT NULL AND done >= enqueue THEN (done - enqueue) ELSE NULL END) AS avg_duration,
                MAX(enqueue) AS last_enqueue,
                SUM(total_tokens) AS total_tokens
            FROM request
            GROUP BY user
            ORDER BY total_requests DESC
            ",
        )
        .fetch_all(get_executor())
        .await?
    };

    Ok(rows
        .into_iter()
        .map(|r| UserMetrics {
            user: r.user.unwrap_or_else(|| "anonymous".to_string()),
            total_requests: r.total_requests,
            pending_requests: r.pending_requests,
            completed_requests: r.completed_requests,
            avg_duration_secs: r.avg_duration,
            last_request_enqueue: r.last_enqueue,
            total_tokens: r.total_tokens.unwrap_or(0),
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct TimeseriesRow {
    bucket: i64,
    request_count: i64,
    avg_duration: Option<f64>,
    total_tokens: Option<i64>,
}

pub async fn get_timeseries_metrics(
    interval_secs: i64,
    points: usize,
) -> Result<Vec<TimeseriesPoint>, anyhow::Error> {
    let interval = interval_secs.max(10);
    let now = Utc::now().timestamp();
    let points_i64 = i64::try_from(points).unwrap_or(288);
    let start_time = now - (interval * points_i64);

    let query = format!(
        r"
        SELECT
            (enqueue / {interval}) * {interval} AS bucket,
            COUNT(*) AS request_count,
            AVG(CASE WHEN done IS NOT NULL AND done >= enqueue THEN (done - enqueue) ELSE NULL END) AS avg_duration,
            SUM(total_tokens) AS total_tokens
        FROM request
        WHERE enqueue >= {start_time}
        GROUP BY bucket
        ORDER BY bucket ASC
        "
    );

    let rows = sqlx::query_as::<_, TimeseriesRow>(&query)
        .fetch_all(get_executor())
        .await?;

    Ok(rows
        .into_iter()
        .map(|r| TimeseriesPoint {
            timestamp: r.bucket,
            request_count: r.request_count,
            avg_duration_secs: r.avg_duration,
            total_tokens: r.total_tokens.unwrap_or(0),
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelLiveStats {
    pub total_queue_length: i64,
    pub user_queue_length: i64,
    pub total_output_tps: f64,
    pub user_output_tps: f64,
    pub user_tps_share_pct: f64,
    pub avg_duration_secs: Option<f64>,
    pub user_total_requests: i64,
    pub user_total_tokens: i64,
    pub active_instances: i64,
}

#[derive(sqlx::FromRow)]
struct PendingRow {
    model: String,
    total_pending: i64,
    user_pending: i64,
}

#[derive(sqlx::FromRow)]
struct WindowTokensRow {
    model: String,
    total_completion_tokens: i64,
    user_completion_tokens: i64,
}

#[derive(sqlx::FromRow)]
struct AvgDurationRow {
    model: String,
    avg_duration: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct UserLifetimeRow {
    model: String,
    user_requests: i64,
    user_tokens: i64,
}

#[derive(sqlx::FromRow)]
struct InstanceCountRow {
    model: String,
    instance_count: i64,
}

pub async fn get_batch_models_live_stats(
    model_ids: &[String],
    username: &str,
    window_secs: i64,
) -> Result<std::collections::HashMap<String, ModelLiveStats>, anyhow::Error> {
    let now = Utc::now().timestamp();
    let window_start = now - window_secs.max(10);
    let avg_window_start = now - 3600; // last 1 hour for average duration

    let mut result: std::collections::HashMap<String, ModelLiveStats> = model_ids
        .iter()
        .map(|id| (id.clone(), ModelLiveStats::default()))
        .collect();

    // 1. Pending queues
    let pending_rows = sqlx::query_as::<_, PendingRow>(
        r"
        SELECT
            model,
            COUNT(DISTINCT id) AS total_pending,
            COALESCE(SUM(CASE WHEN user = ? THEN 1 ELSE 0 END), 0) AS user_pending
        FROM request
        WHERE done IS NULL
        GROUP BY model
        ",
    )
    .bind(username)
    .fetch_all(get_executor())
    .await
    .unwrap_or_default();

    for row in pending_rows {
        if let Some(stats) = result.get_mut(&row.model) {
            stats.total_queue_length = row.total_pending;
            stats.user_queue_length = row.user_pending;
        }
    }

    // 2. Window tokens
    let token_rows = sqlx::query_as::<_, WindowTokensRow>(
        r"
        SELECT
            model,
            COALESCE(SUM(completion_tokens), 0) AS total_completion_tokens,
            COALESCE(SUM(CASE WHEN user = ? THEN completion_tokens ELSE 0 END), 0) AS user_completion_tokens
        FROM request
        WHERE done IS NOT NULL AND done >= ?
        GROUP BY model
        ",
    )
    .bind(username)
    .bind(window_start)
    .fetch_all(get_executor())
    .await
    .unwrap_or_default();

    for row in token_rows {
        if let Some(stats) = result.get_mut(&row.model) {
            let win_f = window_secs.max(1) as f64;
            let total_tps = (row.total_completion_tokens as f64) / win_f;
            let user_tps = (row.user_completion_tokens as f64) / win_f;
            stats.total_output_tps = (total_tps * 100.0).round() / 100.0;
            stats.user_output_tps = (user_tps * 100.0).round() / 100.0;
            stats.user_tps_share_pct = if total_tps > 0.0 {
                ((user_tps / total_tps) * 1000.0).round() / 10.0
            } else {
                0.0
            };
        }
    }

    // 3. Avg duration (last 1h)
    let avg_rows = sqlx::query_as::<_, AvgDurationRow>(
        r"
        SELECT
            model,
            AVG(done - enqueue) AS avg_duration
        FROM request
        WHERE done IS NOT NULL AND done >= enqueue AND enqueue >= ?
        GROUP BY model
        ",
    )
    .bind(avg_window_start)
    .fetch_all(get_executor())
    .await
    .unwrap_or_default();

    for row in avg_rows {
        if let Some(stats) = result.get_mut(&row.model) {
            stats.avg_duration_secs = row.avg_duration;
        }
    }

    // 4. User lifetime requests and tokens
    let user_rows = sqlx::query_as::<_, UserLifetimeRow>(
        r"
        SELECT
            model,
            COUNT(*) AS user_requests,
            COALESCE(SUM(total_tokens), 0) AS user_tokens
        FROM request
        WHERE user = ?
        GROUP BY model
        ",
    )
    .bind(username)
    .fetch_all(get_executor())
    .await
    .unwrap_or_default();

    for row in user_rows {
        if let Some(stats) = result.get_mut(&row.model) {
            stats.user_total_requests = row.user_requests;
            stats.user_total_tokens = row.user_tokens;
        }
    }

    // 5. Active instances
    let inst_rows = sqlx::query_as::<_, InstanceCountRow>(
        r"
        SELECT
            model,
            COUNT(*) AS instance_count
        FROM instance
        GROUP BY model
        ",
    )
    .fetch_all(get_executor())
    .await
    .unwrap_or_default();

    for row in inst_rows {
        if let Some(stats) = result.get_mut(&row.model) {
            stats.active_instances = row.instance_count;
        }
    }

    Ok(result)
}
