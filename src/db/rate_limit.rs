use crate::db::connection::get_executor;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitRule {
    pub id: String,
    pub user: Option<String>,
    pub token_hash: Option<String>,
    pub model: Option<String>,
    pub time_window: String,
    pub limit_value: i64,
    pub unit: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateRateLimitPayload {
    pub user: Option<String>,
    pub token_hash: Option<String>,
    pub model: Option<String>,
    pub time_window: String,
    pub limit_value: i64,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateRateLimitPayload {
    pub user: Option<String>,
    pub token_hash: Option<String>,
    pub model: Option<String>,
    pub time_window: String,
    pub limit_value: i64,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitUsageInfo {
    pub id: String,
    pub user: Option<String>,
    pub token_hash: Option<String>,
    pub model: Option<String>,
    pub time_window: String,
    pub limit_value: i64,
    pub unit: String,
    pub used: i64,
    pub remaining: i64,
    pub reset_in_seconds: Option<i64>,
    pub reset_timestamp: Option<i64>,
}

pub fn window_duration_seconds(time_window: &str) -> Option<i64> {
    match time_window.to_lowercase().as_str() {
        "hour" | "1h" | "h" => Some(3600),
        "day" | "1d" | "d" => Some(86_400),
        "week" | "1w" | "w" => Some(604_800),
        "month" | "1mo" | "mo" | "30d" => Some(2_592_000), // 30 days
        "permanent" | "lifetime" | "total" => None,
        _ => Some(60), // default to minute
    }
}

pub fn normalize_time_window(time_window: &str) -> String {
    match time_window.to_lowercase().as_str() {
        "minute" | "1m" | "m" => "minute".to_string(),
        "hour" | "1h" | "h" => "hour".to_string(),
        "day" | "1d" | "d" => "day".to_string(),
        "week" | "1w" | "w" => "week".to_string(),
        "month" | "1mo" | "mo" | "30d" => "month".to_string(),
        "permanent" | "lifetime" | "total" => "permanent".to_string(),
        other => other.to_string(),
    }
}

pub fn normalize_unit(unit: Option<&str>) -> String {
    match unit.map(str::to_lowercase).as_deref() {
        Some("requests" | "req" | "reqs") => "requests".to_string(),
        Some("prompt_tokens" | "input_tokens") => "prompt_tokens".to_string(),
        Some("completion_tokens" | "output_tokens") => "completion_tokens".to_string(),
        _ => "tokens".to_string(),
    }
}

pub async fn create_rate_limit(payload: &CreateRateLimitPayload) -> Result<RateLimitRule, anyhow::Error> {
    let id = Uuid::new_v4().to_string();
    let time_window = normalize_time_window(&payload.time_window);
    let unit = normalize_unit(payload.unit.as_deref());
    let user = payload.user.as_deref().and_then(|u| {
        let trimmed = u.trim();
        if trimmed.is_empty() || trimmed == "*" {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let model = payload.model.as_deref().and_then(|m| {
        let trimmed = m.trim();
        if trimmed.is_empty() || trimmed == "*" {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    let token_hash = payload.token_hash.as_deref().and_then(|t| {
        let trimmed = t.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    let now = Utc::now().timestamp();

    sqlx::query(
        r"
        INSERT INTO rate_limits (id, user, token_hash, model, time_window, limit_value, unit, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ",
    )
    .bind(&id)
    .bind(&user)
    .bind(&token_hash)
    .bind(&model)
    .bind(&time_window)
    .bind(payload.limit_value)
    .bind(&unit)
    .bind(now)
    .execute(get_executor())
    .await?;

    Ok(RateLimitRule {
        id,
        user,
        token_hash,
        model,
        time_window,
        limit_value: payload.limit_value,
        unit,
        created_at: now,
    })
}

pub async fn update_rate_limit(
    id: &str,
    payload: &UpdateRateLimitPayload,
) -> Result<Option<RateLimitRule>, anyhow::Error> {
    let time_window = normalize_time_window(&payload.time_window);
    let unit = normalize_unit(payload.unit.as_deref());
    let user = payload.user.as_deref().and_then(|u| {
        let trimmed = u.trim();
        if trimmed.is_empty() || trimmed == "*" {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let model = payload.model.as_deref().and_then(|m| {
        let trimmed = m.trim();
        if trimmed.is_empty() || trimmed == "*" {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    let token_hash = payload.token_hash.as_deref().and_then(|t| {
        let trimmed = t.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    let res = sqlx::query(
        r"
        UPDATE rate_limits
        SET user = ?, token_hash = ?, model = ?, time_window = ?, limit_value = ?, unit = ?
        WHERE id = ?
        ",
    )
    .bind(&user)
    .bind(&token_hash)
    .bind(&model)
    .bind(&time_window)
    .bind(payload.limit_value)
    .bind(&unit)
    .bind(id)
    .execute(get_executor())
    .await?;

    if res.rows_affected() == 0 {
        return Ok(None);
    }

    get_rate_limit_by_id(id).await
}

pub async fn delete_rate_limit(id: &str) -> Result<bool, anyhow::Error> {
    let res = sqlx::query("DELETE FROM rate_limits WHERE id = ?")
        .bind(id)
        .execute(get_executor())
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn get_rate_limit_by_id(id: &str) -> Result<Option<RateLimitRule>, anyhow::Error> {
    let rule = sqlx::query_as::<_, RateLimitRule>(
        "SELECT id, user, token_hash, model, time_window, limit_value, unit, created_at FROM rate_limits WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(get_executor())
    .await?;

    Ok(rule)
}

pub async fn get_all_rate_limits(
    user_filter: Option<&str>,
    model_filter: Option<&str>,
) -> Result<Vec<RateLimitRule>, anyhow::Error> {
    let mut query = "SELECT id, user, token_hash, model, time_window, limit_value, unit, created_at FROM rate_limits".to_string();
    let mut where_clauses = Vec::new();

    if let Some(u) = user_filter {
        if u == "*" {
            where_clauses.push("user IS NULL".to_string());
        } else {
            where_clauses.push(format!("user = '{}'", u.replace('\'', "''")));
        }
    }

    if let Some(m) = model_filter {
        if m == "*" {
            where_clauses.push("model IS NULL".to_string());
        } else {
            where_clauses.push(format!("model = '{}'", m.replace('\'', "''")));
        }
    }

    if !where_clauses.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&where_clauses.join(" AND "));
    }

    query.push_str(" ORDER BY created_at DESC");

    let rules = sqlx::query_as::<_, RateLimitRule>(&query)
        .fetch_all(get_executor())
        .await?;

    Ok(rules)
}

pub async fn get_matching_rate_limits(
    username: &str,
    model_name: &str,
) -> Result<Vec<RateLimitRule>, anyhow::Error> {
    let rules = sqlx::query_as::<_, RateLimitRule>(
        r"
        SELECT id, user, token_hash, model, time_window, limit_value, unit, created_at
        FROM rate_limits
        WHERE token_hash IS NULL
          AND (user = ? OR user IS NULL OR user = '*')
        ORDER BY created_at ASC
        ",
    )
    .bind(username)
    .fetch_all(get_executor())
    .await?;

    let filtered = rules
        .into_iter()
        .filter(|rule| match &rule.model {
            None => true,
            Some(m) if m == "*" || m == model_name => true,
            Some(m) => crate::utils::matches_glob(m, model_name),
        })
        .collect();

    Ok(filtered)
}

pub async fn get_matching_token_rate_limits(
    token_hash: &str,
    model_name: &str,
) -> Result<Vec<RateLimitRule>, anyhow::Error> {
    let rules = sqlx::query_as::<_, RateLimitRule>(
        r"
        SELECT id, user, token_hash, model, time_window, limit_value, unit, created_at
        FROM rate_limits
        WHERE token_hash = ?
        ORDER BY created_at ASC
        ",
    )
    .bind(token_hash)
    .fetch_all(get_executor())
    .await?;

    let filtered = rules
        .into_iter()
        .filter(|rule| match &rule.model {
            None => true,
            Some(m) if m == "*" || m == model_name => true,
            Some(m) => crate::utils::matches_glob(m, model_name),
        })
        .collect();

    Ok(filtered)
}

pub async fn get_rate_limits_for_token(
    token_hash: &str,
) -> Result<Vec<RateLimitRule>, anyhow::Error> {
    let rules = sqlx::query_as::<_, RateLimitRule>(
        r"
        SELECT id, user, token_hash, model, time_window, limit_value, unit, created_at
        FROM rate_limits
        WHERE token_hash = ?
        ORDER BY created_at ASC
        ",
    )
    .bind(token_hash)
    .fetch_all(get_executor())
    .await?;

    Ok(rules)
}

pub async fn delete_rate_limits_for_token(
    token_hash: &str,
) -> Result<(), anyhow::Error> {
    sqlx::query("DELETE FROM rate_limits WHERE token_hash = ?")
        .bind(token_hash)
        .execute(get_executor())
        .await?;
    Ok(())
}

pub async fn calculate_usage_for_rule(
    rule: &RateLimitRule,
    username: &str,
    model_name: &str,
    now: i64,
) -> Result<RateLimitUsageInfo, anyhow::Error> {
    let duration = window_duration_seconds(&rule.time_window);
    let start_time = match duration {
        Some(dur) => now - dur,
        None => 0, // permanent / lifetime
    };

    let target_user = username;
    let is_model_specific = rule.model.as_ref().is_some_and(|m| m != "*" && !m.is_empty());
    let target_model = if is_model_specific {
        rule.model.as_deref().unwrap_or(model_name)
    } else {
        model_name
    };

    let column_expr = match rule.unit.as_str() {
        "requests" => "COUNT(*)",
        "prompt_tokens" => "COALESCE(SUM(prompt_tokens), 0)",
        "completion_tokens" => "COALESCE(SUM(completion_tokens), 0)",
        _ => "COALESCE(SUM(total_tokens), 0)",
    };

    let used: i64 = if let Some(ref th) = rule.token_hash {
        if is_model_specific {
            let query = format!(
                "SELECT {column_expr} FROM request WHERE token_hash = ? AND model = ? AND enqueue >= ?"
            );
            sqlx::query_scalar(&query)
                .bind(th)
                .bind(target_model)
                .bind(start_time)
                .fetch_one(get_executor())
                .await
                .unwrap_or(0)
        } else {
            let query = format!(
                "SELECT {column_expr} FROM request WHERE token_hash = ? AND enqueue >= ?"
            );
            sqlx::query_scalar(&query)
                .bind(th)
                .bind(start_time)
                .fetch_one(get_executor())
                .await
                .unwrap_or(0)
        }
    } else if is_model_specific {
        let query = format!(
            "SELECT {column_expr} FROM request WHERE user = ? AND model = ? AND enqueue >= ?"
        );
        sqlx::query_scalar(&query)
            .bind(target_user)
            .bind(target_model)
            .bind(start_time)
            .fetch_one(get_executor())
            .await
            .unwrap_or(0)
    } else {
        let query = format!(
            "SELECT {column_expr} FROM request WHERE user = ? AND enqueue >= ?"
        );
        sqlx::query_scalar(&query)
            .bind(target_user)
            .bind(start_time)
            .fetch_one(get_executor())
            .await
            .unwrap_or(0)
    };

    let remaining = (rule.limit_value - used).max(0);

    let (reset_in_seconds, reset_timestamp) = if let Some(dur) = duration {
        let oldest_in_window: Option<i64> = if let Some(ref th) = rule.token_hash {
            if is_model_specific {
                sqlx::query_scalar(
                    "SELECT MIN(enqueue) FROM request WHERE token_hash = ? AND model = ? AND enqueue >= ?",
                )
                .bind(th)
                .bind(target_model)
                .bind(start_time)
                .fetch_optional(get_executor())
                .await
                .unwrap_or(None)
            } else {
                sqlx::query_scalar(
                    "SELECT MIN(enqueue) FROM request WHERE token_hash = ? AND enqueue >= ?",
                )
                .bind(th)
                .bind(start_time)
                .fetch_optional(get_executor())
                .await
                .unwrap_or(None)
            }
        } else if is_model_specific {
            sqlx::query_scalar(
                "SELECT MIN(enqueue) FROM request WHERE user = ? AND model = ? AND enqueue >= ?",
            )
            .bind(target_user)
            .bind(target_model)
            .bind(start_time)
            .fetch_optional(get_executor())
            .await
            .unwrap_or(None)
        } else {
            sqlx::query_scalar(
                "SELECT MIN(enqueue) FROM request WHERE user = ? AND enqueue >= ?",
            )
            .bind(target_user)
            .bind(start_time)
            .fetch_optional(get_executor())
            .await
            .unwrap_or(None)
        };

        if let Some(oldest) = oldest_in_window {
            let reset_ts = oldest + dur;
            let secs = (reset_ts - now).max(1);
            (Some(secs), Some(reset_ts))
        } else {
            (Some(dur), Some(now + dur))
        }
    } else {
        (None, None)
    };

    Ok(RateLimitUsageInfo {
        id: rule.id.clone(),
        user: rule.user.clone(),
        token_hash: rule.token_hash.clone(),
        model: rule.model.clone(),
        time_window: rule.time_window.clone(),
        limit_value: rule.limit_value,
        unit: rule.unit.clone(),
        used,
        remaining,
        reset_in_seconds,
        reset_timestamp,
    })
}
