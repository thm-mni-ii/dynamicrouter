use crate::db::rate_limit::{
    calculate_usage_for_rule, get_matching_rate_limits_for_user_and_groups,
    get_matching_token_rate_limits, normalize_unit, window_duration_seconds,
};
use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn estimate_prompt_tokens(request: &Value) -> i64 {
    // 1. If messages array is present (standard ChatCompletion)
    if let Some(messages) = request.get("messages").and_then(Value::as_array) {
        let mut char_count = 0;
        for msg in messages {
            if let Some(content) = msg.get("content").and_then(Value::as_str) {
                char_count += content.chars().count();
            }
            if let Some(role) = msg.get("role").and_then(Value::as_str) {
                char_count += role.chars().count() + 4;
            }
        }
        let estimated = ((char_count as f64) / 3.8).ceil() as i64 + 4;
        return estimated.max(1);
    }

    // 2. If prompt string / array is present (standard Completion)
    if let Some(prompt) = request.get("prompt") {
        if let Some(s) = prompt.as_str() {
            let estimated = ((s.chars().count() as f64) / 3.8).ceil() as i64;
            return estimated.max(1);
        }
        if let Some(arr) = prompt.as_array() {
            let mut total_chars = 0;
            for p in arr {
                if let Some(s) = p.as_str() {
                    total_chars += s.chars().count();
                }
            }
            let estimated = ((total_chars as f64) / 3.8).ceil() as i64;
            return estimated.max(1);
        }
    }

    // 3. If input is present (standard Embeddings)
    if let Some(input) = request.get("input") {
        if let Some(s) = input.as_str() {
            let estimated = ((s.chars().count() as f64) / 3.8).ceil() as i64;
            return estimated.max(1);
        }
        if let Some(arr) = input.as_array() {
            let mut total_chars = 0;
            for item in arr {
                if let Some(s) = item.as_str() {
                    total_chars += s.chars().count();
                }
            }
            let estimated = ((total_chars as f64) / 3.8).ceil() as i64;
            return estimated.max(1);
        }
    }

    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitRejection {
    pub message: String,
    pub headers: Vec<(String, String)>,
    pub rate_limit_info: Value,
}

impl IntoResponse for RateLimitRejection {
    fn into_response(self) -> Response {
        let mut builder = Response::builder().status(StatusCode::TOO_MANY_REQUESTS);

        for (name, val) in &self.headers {
            if let (Ok(h_name), Ok(h_val)) = (
                axum::http::HeaderName::from_bytes(name.as_bytes()),
                axum::http::HeaderValue::from_str(val),
            ) {
                builder = builder.header(h_name, h_val);
            }
        }

        builder = builder.header(header::CONTENT_TYPE, "application/json");

        let body_json = serde_json::json!({
            "error": {
                "message": self.message,
                "type": "rate_limit_error",
                "code": "rate_limit_exceeded",
                "param": null,
                "rate_limit": self.rate_limit_info,
            }
        });

        builder
            .body(Body::from(
                serde_json::to_vec(&body_json).unwrap_or_default(),
            ))
            .unwrap()
    }
}

pub async fn check_rate_limits_for_user_and_groups(
    username: &str,
    groups: &[String],
    model_name: &str,
    prompt_tokens: i64,
) -> Result<Vec<(String, String)>, RateLimitRejection> {
    let rules =
        match get_matching_rate_limits_for_user_and_groups(username, groups, model_name).await {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to query rate limit rules for {username}: {e}");
                return Ok(Vec::new());
            }
        };

    if rules.is_empty() {
        return Ok(Vec::new());
    }

    let now = Utc::now().timestamp();
    let mut most_constrained_limit = None;
    let mut min_remaining_ratio = f64::MAX;

    for rule in &rules {
        let usage = match calculate_usage_for_rule(rule, username, model_name, now).await {
            Ok(u) => u,
            Err(e) => {
                warn!("Failed to calculate usage for rule {}: {e}", rule.id);
                continue;
            }
        };

        let unit_norm = normalize_unit(Some(&rule.unit));
        let additional_cost = match unit_norm.as_str() {
            "requests" => 1,
            "completion_tokens" => 0, // completion tokens are added after generation
            _ => prompt_tokens,
        };

        let projected_usage = usage.used + additional_cost;
        let is_exceeded = if unit_norm == "requests" {
            usage.used >= rule.limit_value
        } else {
            projected_usage > rule.limit_value || usage.used >= rule.limit_value
        };

        if is_exceeded {
            let reset_secs = usage.reset_in_seconds.unwrap_or(0);
            let reset_ts = usage.reset_timestamp.unwrap_or(now + reset_secs);

            let mut headers = vec![
                (
                    "X-RateLimit-Limit".to_string(),
                    rule.limit_value.to_string(),
                ),
                ("X-RateLimit-Remaining".to_string(), "0".to_string()),
                ("X-RateLimit-Reset".to_string(), reset_ts.to_string()),
                (
                    "X-RateLimit-Scope".to_string(),
                    format!(
                        "user={},model={},window={},unit={}",
                        rule.user.as_deref().unwrap_or("*"),
                        rule.model.as_deref().unwrap_or("*"),
                        rule.time_window,
                        rule.unit
                    ),
                ),
            ];

            if reset_secs > 0 {
                headers.push(("Retry-After".to_string(), reset_secs.to_string()));
            }

            let unit_display = match unit_norm.as_str() {
                "requests" => "requests",
                "prompt_tokens" => "prompt tokens",
                "completion_tokens" => "completion tokens",
                _ => "tokens",
            };

            let time_display = match rule.time_window.as_str() {
                "minute" => "minute",
                "hour" => "hour",
                "day" => "day",
                "week" => "week",
                "month" => "month",
                "permanent" => "permanent lifetime",
                _ => &rule.time_window,
            };

            let target_model_display = rule.model.as_deref().unwrap_or("all models");
            let message = if rule.time_window == "permanent" {
                format!(
                    "Rate limit exceeded for user '{username}' on {target_model_display}: permanent limit of {} {unit_display} reached (used: {} {unit_display}).",
                    rule.limit_value, usage.used
                )
            } else {
                format!(
                    "Rate limit exceeded for user '{username}' on {target_model_display}: limit of {} {unit_display} per {time_display} reached (used: {}, requested prompt: {prompt_tokens}). Please retry in {reset_secs} seconds.",
                    rule.limit_value, usage.used
                )
            };

            let rate_limit_info = serde_json::json!({
                "user": username,
                "model": model_name,
                "window": rule.time_window,
                "unit": rule.unit,
                "limit": rule.limit_value,
                "used": usage.used,
                "requested_prompt_tokens": prompt_tokens,
                "reset_in_seconds": reset_secs,
                "reset_timestamp": reset_ts,
            });

            return Err(RateLimitRejection {
                message,
                headers,
                rate_limit_info,
            });
        }

        // Track the most constrained rule for successful response headers
        let remaining_after = (rule.limit_value - projected_usage).max(0);
        let ratio = if rule.limit_value > 0 {
            (remaining_after as f64) / (rule.limit_value as f64)
        } else {
            0.0
        };

        if ratio < min_remaining_ratio {
            min_remaining_ratio = ratio;
            let reset_ts = usage
                .reset_timestamp
                .unwrap_or(now + window_duration_seconds(&rule.time_window).unwrap_or(60));

            most_constrained_limit = Some((
                rule.limit_value,
                remaining_after,
                reset_ts,
                rule.user.as_deref().unwrap_or("*"),
                rule.model.as_deref().unwrap_or("*"),
                &rule.time_window,
                &rule.unit,
            ));
        }
    }

    let mut response_headers = Vec::new();
    if let Some((limit, remaining, reset_ts, user_sc, model_sc, window_sc, unit_sc)) =
        most_constrained_limit
    {
        response_headers.push(("X-RateLimit-Limit".to_string(), limit.to_string()));
        response_headers.push(("X-RateLimit-Remaining".to_string(), remaining.to_string()));
        response_headers.push(("X-RateLimit-Reset".to_string(), reset_ts.to_string()));
        response_headers.push((
            "X-RateLimit-Scope".to_string(),
            format!("user={user_sc},model={model_sc},window={window_sc},unit={unit_sc}"),
        ));
    }

    Ok(response_headers)
}

pub async fn check_rate_limits(
    username: &str,
    model_name: &str,
    prompt_tokens: i64,
) -> Result<Vec<(String, String)>, RateLimitRejection> {
    let groups = crate::db::group::get_user_groups(username)
        .await
        .unwrap_or_default();
    check_rate_limits_for_user_and_groups(username, &groups, model_name, prompt_tokens).await
}

pub async fn check_token_rate_limits(
    token_hash: &str,
    token_name_or_preview: &str,
    model_name: &str,
    prompt_tokens: i64,
) -> Result<Vec<(String, String)>, RateLimitRejection> {
    let rules = match get_matching_token_rate_limits(token_hash, model_name).await {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to query token rate limit rules for {token_hash}: {e}");
            return Ok(Vec::new());
        }
    };

    if rules.is_empty() {
        return Ok(Vec::new());
    }

    let now = Utc::now().timestamp();
    let mut most_constrained_limit = None;
    let mut min_remaining_ratio = f64::MAX;

    for rule in &rules {
        let usage = match calculate_usage_for_rule(rule, "", model_name, now).await {
            Ok(u) => u,
            Err(e) => {
                warn!("Failed to calculate usage for token rule {}: {e}", rule.id);
                continue;
            }
        };

        let unit_norm = normalize_unit(Some(&rule.unit));
        let additional_cost = match unit_norm.as_str() {
            "requests" => 1,
            "completion_tokens" => 0,
            _ => prompt_tokens,
        };

        let projected_usage = usage.used + additional_cost;
        let is_exceeded = if unit_norm == "requests" {
            usage.used >= rule.limit_value
        } else {
            projected_usage > rule.limit_value || usage.used >= rule.limit_value
        };

        if is_exceeded {
            let reset_secs = usage.reset_in_seconds.unwrap_or(0);
            let reset_ts = usage.reset_timestamp.unwrap_or(now + reset_secs);

            let mut headers = vec![
                (
                    "X-RateLimit-Limit".to_string(),
                    rule.limit_value.to_string(),
                ),
                ("X-RateLimit-Remaining".to_string(), "0".to_string()),
                ("X-RateLimit-Reset".to_string(), reset_ts.to_string()),
                (
                    "X-RateLimit-Scope".to_string(),
                    format!(
                        "token={},model={},window={},unit={}",
                        token_name_or_preview,
                        rule.model.as_deref().unwrap_or("*"),
                        rule.time_window,
                        rule.unit
                    ),
                ),
            ];

            if reset_secs > 0 {
                headers.push(("Retry-After".to_string(), reset_secs.to_string()));
            }

            let unit_display = match unit_norm.as_str() {
                "requests" => "requests",
                "prompt_tokens" => "prompt tokens",
                "completion_tokens" => "completion tokens",
                _ => "tokens",
            };

            let time_display = match rule.time_window.as_str() {
                "minute" => "minute",
                "hour" => "hour",
                "day" => "day",
                "week" => "week",
                "month" => "month",
                "permanent" => "permanent lifetime",
                _ => &rule.time_window,
            };

            let target_model_display = rule.model.as_deref().unwrap_or("all models");
            let message = if rule.time_window == "permanent" {
                format!(
                    "Token rate limit exceeded for token '{token_name_or_preview}' on {target_model_display}: permanent limit of {} {unit_display} reached (used: {} {unit_display}).",
                    rule.limit_value, usage.used
                )
            } else {
                format!(
                    "Token rate limit exceeded for token '{token_name_or_preview}' on {target_model_display}: limit of {} {unit_display} per {time_display} reached (used: {}, requested prompt: {prompt_tokens}). Please retry in {reset_secs} seconds.",
                    rule.limit_value, usage.used
                )
            };

            let rate_limit_info = serde_json::json!({
                "token": token_name_or_preview,
                "model": model_name,
                "window": rule.time_window,
                "unit": rule.unit,
                "limit": rule.limit_value,
                "used": usage.used,
                "requested_prompt_tokens": prompt_tokens,
                "reset_in_seconds": reset_secs,
                "reset_timestamp": reset_ts,
            });

            return Err(RateLimitRejection {
                message,
                headers,
                rate_limit_info,
            });
        }

        let remaining_after = (rule.limit_value - projected_usage).max(0);
        let ratio = if rule.limit_value > 0 {
            (remaining_after as f64) / (rule.limit_value as f64)
        } else {
            0.0
        };

        if ratio < min_remaining_ratio {
            min_remaining_ratio = ratio;
            let reset_ts = usage
                .reset_timestamp
                .unwrap_or(now + window_duration_seconds(&rule.time_window).unwrap_or(60));

            most_constrained_limit = Some((
                rule.limit_value,
                remaining_after,
                reset_ts,
                token_name_or_preview,
                rule.model.as_deref().unwrap_or("*"),
                &rule.time_window,
                &rule.unit,
            ));
        }
    }

    let mut response_headers = Vec::new();
    if let Some((limit, remaining, reset_ts, token_sc, model_sc, window_sc, unit_sc)) =
        most_constrained_limit
    {
        response_headers.push(("X-RateLimit-Limit".to_string(), limit.to_string()));
        response_headers.push(("X-RateLimit-Remaining".to_string(), remaining.to_string()));
        response_headers.push(("X-RateLimit-Reset".to_string(), reset_ts.to_string()));
        response_headers.push((
            "X-RateLimit-Scope".to_string(),
            format!("token={token_sc},model={model_sc},window={window_sc},unit={unit_sc}"),
        ));
    }

    Ok(response_headers)
}
