use crate::db::connection::get_executor;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitRule {
    pub id: String,
    pub user: Option<String>,
    pub token_hash: Option<String>,
    pub group_name: Option<String>,
    pub scope_mode: String,
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
    pub group_name: Option<String>,
    pub scope_mode: Option<String>,
    pub model: Option<String>,
    pub time_window: String,
    pub limit_value: i64,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateRateLimitPayload {
    pub user: Option<String>,
    pub token_hash: Option<String>,
    pub group_name: Option<String>,
    pub scope_mode: Option<String>,
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
    pub group_name: Option<String>,
    pub scope_mode: String,
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

pub fn normalize_scope_mode(mode: Option<&str>) -> String {
    match mode.map(str::to_lowercase).as_deref() {
        Some("shared_group" | "shared" | "group") => "shared_group".to_string(),
        _ => "per_user".to_string(),
    }
}

pub fn matches_target_list(pattern_list: &str, candidate: &str) -> bool {
    let cand_trim = candidate.trim();
    for part in pattern_list.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "*" || trimmed == cand_trim || crate::utils::matches_glob(trimmed, cand_trim) {
            return true;
        }
    }
    false
}

pub fn normalize_target_list(input: Option<&str>) -> Option<String> {
    let s = input?.trim();
    if s.is_empty() || s == "*" {
        return None;
    }
    let parts: Vec<&str> = s
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() || parts.iter().any(|&p| p == "*") {
        None
    } else {
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::new();
        for p in parts {
            if seen.insert(p) {
                deduped.push(p);
            }
        }
        Some(deduped.join(", "))
    }
}

pub async fn create_rate_limit(payload: &CreateRateLimitPayload) -> Result<RateLimitRule, anyhow::Error> {
    let id = Uuid::new_v4().to_string();
    let time_window = normalize_time_window(&payload.time_window);
    let unit = normalize_unit(payload.unit.as_deref());
    let user = normalize_target_list(payload.user.as_deref());
    let group_name = normalize_target_list(payload.group_name.as_deref());
    let scope_mode = normalize_scope_mode(payload.scope_mode.as_deref());
    let model = normalize_target_list(payload.model.as_deref());

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
        INSERT INTO rate_limits (id, user, token_hash, group_name, scope_mode, model, time_window, limit_value, unit, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ",
    )
    .bind(&id)
    .bind(&user)
    .bind(&token_hash)
    .bind(&group_name)
    .bind(&scope_mode)
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
        group_name,
        scope_mode,
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
    let user = normalize_target_list(payload.user.as_deref());
    let group_name = normalize_target_list(payload.group_name.as_deref());
    let scope_mode = normalize_scope_mode(payload.scope_mode.as_deref());
    let model = normalize_target_list(payload.model.as_deref());

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
        SET user = ?, token_hash = ?, group_name = ?, scope_mode = ?, model = ?, time_window = ?, limit_value = ?, unit = ?
        WHERE id = ?
        ",
    )
    .bind(&user)
    .bind(&token_hash)
    .bind(&group_name)
    .bind(&scope_mode)
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
        "SELECT id, user, token_hash, group_name, scope_mode, model, time_window, limit_value, unit, created_at FROM rate_limits WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(get_executor())
    .await?;

    Ok(rule)
}

pub async fn get_all_rate_limits(
    user_filter: Option<&str>,
    model_filter: Option<&str>,
    group_filter: Option<&str>,
) -> Result<Vec<RateLimitRule>, anyhow::Error> {
    let rules = sqlx::query_as::<_, RateLimitRule>(
        "SELECT id, user, token_hash, group_name, scope_mode, model, time_window, limit_value, unit, created_at FROM rate_limits ORDER BY created_at DESC",
    )
    .fetch_all(get_executor())
    .await?;

    let filtered = rules
        .into_iter()
        .filter(|rule| {
            if let Some(u) = user_filter {
                if u == "*" {
                    if rule.user.is_some() {
                        return false;
                    }
                } else if !rule.user.as_deref().map_or(false, |pat| matches_target_list(pat, u)) {
                    return false;
                }
            }
            if let Some(m) = model_filter {
                if m == "*" {
                    if rule.model.is_some() {
                        return false;
                    }
                } else if !rule.model.as_deref().map_or(false, |pat| matches_target_list(pat, m)) {
                    return false;
                }
            }
            if let Some(g) = group_filter {
                if g == "*" {
                    if rule.group_name.is_some() {
                        return false;
                    }
                } else if !rule.group_name.as_deref().map_or(false, |pat| matches_target_list(pat, g)) {
                    return false;
                }
            }
            true
        })
        .collect();

    Ok(filtered)
}

pub async fn get_matching_rate_limits_for_user_and_groups(
    username: &str,
    groups: &[String],
    model_name: &str,
) -> Result<Vec<RateLimitRule>, anyhow::Error> {
    let rules = sqlx::query_as::<_, RateLimitRule>(
        r"
        SELECT id, user, token_hash, group_name, scope_mode, model, time_window, limit_value, unit, created_at
        FROM rate_limits
        WHERE token_hash IS NULL
        ORDER BY created_at ASC
        ",
    )
    .fetch_all(get_executor())
    .await?;

    let filtered = rules
        .into_iter()
        .filter(|rule| {
            let target_matches = match (&rule.user, &rule.group_name) {
                (None, None) => true,
                (Some(u), None) => matches_target_list(u, username),
                (None, Some(g)) => groups.iter().any(|ug| matches_target_list(g, ug)),
                (Some(u), Some(g)) => {
                    matches_target_list(u, username) && groups.iter().any(|ug| matches_target_list(g, ug))
                }
            };
            if !target_matches {
                return false;
            }
            match &rule.model {
                None => true,
                Some(m) => matches_target_list(m, model_name),
            }
        })
        .collect();

    Ok(filtered)
}

pub async fn get_matching_rate_limits(
    username: &str,
    model_name: &str,
) -> Result<Vec<RateLimitRule>, anyhow::Error> {
    let groups = crate::db::group::get_user_manual_groups(username)
        .await
        .unwrap_or_default();
    get_matching_rate_limits_for_user_and_groups(username, &groups, model_name).await
}

pub async fn get_rate_limits_for_user_and_groups(
    username: &str,
    groups: &[String],
) -> Result<Vec<RateLimitRule>, anyhow::Error> {
    let rules = sqlx::query_as::<_, RateLimitRule>(
        r"
        SELECT id, user, token_hash, group_name, scope_mode, model, time_window, limit_value, unit, created_at
        FROM rate_limits
        WHERE token_hash IS NULL
        ORDER BY created_at ASC
        ",
    )
    .fetch_all(get_executor())
    .await?;

    let filtered = rules
        .into_iter()
        .filter(|rule| match (&rule.user, &rule.group_name) {
            (None, None) => true,
            (Some(u), None) => matches_target_list(u, username),
            (None, Some(g)) => groups.iter().any(|ug| matches_target_list(g, ug)),
            (Some(u), Some(g)) => {
                matches_target_list(u, username) && groups.iter().any(|ug| matches_target_list(g, ug))
            }
        })
        .collect();

    Ok(filtered)
}

pub async fn get_rate_limits_for_user(
    username: &str,
) -> Result<Vec<RateLimitRule>, anyhow::Error> {
    let groups = crate::db::group::get_user_manual_groups(username)
        .await
        .unwrap_or_default();
    get_rate_limits_for_user_and_groups(username, &groups).await
}

pub async fn get_rate_limits_strictly_for_user(
    username: &str,
) -> Result<Vec<RateLimitRule>, anyhow::Error> {
    let rules = sqlx::query_as::<_, RateLimitRule>(
        r"
        SELECT id, user, token_hash, group_name, scope_mode, model, time_window, limit_value, unit, created_at
        FROM rate_limits
        WHERE token_hash IS NULL
        ORDER BY created_at ASC
        ",
    )
    .fetch_all(get_executor())
    .await?;

    let filtered = rules
        .into_iter()
        .filter(|rule| match &rule.user {
            Some(u) => matches_target_list(u, username),
            None => false,
        })
        .collect();

    Ok(filtered)
}

pub async fn delete_rate_limits_for_user(
    username: &str,
) -> Result<(), anyhow::Error> {
    let rules = sqlx::query_as::<_, RateLimitRule>(
        "SELECT id, user, token_hash, group_name, scope_mode, model, time_window, limit_value, unit, created_at FROM rate_limits WHERE token_hash IS NULL AND user IS NOT NULL",
    )
    .fetch_all(get_executor())
    .await?;

    for rule in rules {
        if let Some(ref u) = rule.user {
            let parts: Vec<&str> = u.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
            if parts.contains(&username) {
                let remaining: Vec<&str> = parts.into_iter().filter(|&p| p != username).collect();
                if remaining.is_empty() {
                    sqlx::query("DELETE FROM rate_limits WHERE id = ?")
                        .bind(&rule.id)
                        .execute(get_executor())
                        .await?;
                } else {
                    let updated_user = remaining.join(", ");
                    sqlx::query("UPDATE rate_limits SET user = ? WHERE id = ?")
                        .bind(&updated_user)
                        .bind(&rule.id)
                        .execute(get_executor())
                        .await?;
                }
            }
        }
    }
    Ok(())
}

pub async fn get_matching_token_rate_limits(
    token_hash: &str,
    model_name: &str,
) -> Result<Vec<RateLimitRule>, anyhow::Error> {
    let rules = sqlx::query_as::<_, RateLimitRule>(
        r"
        SELECT id, user, token_hash, group_name, scope_mode, model, time_window, limit_value, unit, created_at
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
        SELECT id, user, token_hash, group_name, scope_mode, model, time_window, limit_value, unit, created_at
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

#[derive(Clone)]
enum ModelFilter<'a> {
    ModelList(Vec<&'a str>),
    Any,
}

#[derive(Clone)]
enum UserOrTokenFilter<'a> {
    Token(&'a str),
    UserList(Vec<&'a str>),
    Any,
}

async fn query_request_scalar_i64(
    column_expr: &str,
    target: &UserOrTokenFilter<'_>,
    model: &ModelFilter<'_>,
    start_time: i64,
) -> Option<i64> {
    let pool = get_executor();
    let mut where_parts: Vec<String> = Vec::new();
    let mut string_binds: Vec<String> = Vec::new();

    match target {
        UserOrTokenFilter::Token(th) => {
            where_parts.push("token_hash = ?".to_string());
            string_binds.push(th.to_string());
        }
        UserOrTokenFilter::UserList(users) if !users.is_empty() => {
            let mut user_clauses = Vec::new();
            for u in users {
                if crate::utils::is_glob_pattern(u) {
                    user_clauses.push("user GLOB ?".to_string());
                } else {
                    user_clauses.push("user = ?".to_string());
                }
                string_binds.push(u.to_string());
            }
            where_parts.push(format!("({})", user_clauses.join(" OR ")));
        }
        _ => {}
    }

    match model {
        ModelFilter::ModelList(models) if !models.is_empty() => {
            let mut model_clauses = Vec::new();
            for m in models {
                if crate::utils::is_glob_pattern(m) {
                    model_clauses.push("model GLOB ?".to_string());
                } else {
                    model_clauses.push("model = ?".to_string());
                }
                string_binds.push(m.to_string());
            }
            where_parts.push(format!("({})", model_clauses.join(" OR ")));
        }
        _ => {}
    }

    where_parts.push("enqueue >= ?".to_string());

    let q_str = format!(
        "SELECT {} FROM request WHERE {}",
        column_expr,
        where_parts.join(" AND ")
    );

    let mut query = sqlx::query_scalar::<_, i64>(&q_str);
    for s in string_binds {
        query = query.bind(s);
    }
    query.bind(start_time).fetch_optional(pool).await.ok().flatten()
}

pub async fn calculate_usage_for_rule(
    rule: &RateLimitRule,
    username: &str,
    _model_name: &str,
    now: i64,
) -> Result<RateLimitUsageInfo, anyhow::Error> {
    let duration = window_duration_seconds(&rule.time_window);
    let start_time = match duration {
        Some(dur) => now - dur,
        None => 0, // permanent / lifetime
    };

    let target_user = username.trim();

    let model_filter = match &rule.model {
        Some(m) if m != "*" && !m.trim().is_empty() => {
            let items: Vec<&str> = m
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty() && *s != "*")
                .collect();
            if items.is_empty() {
                ModelFilter::Any
            } else {
                ModelFilter::ModelList(items)
            }
        }
        _ => ModelFilter::Any,
    };

    let shared_group_members: Vec<String>;
    let target_filter = if let Some(ref th) = rule.token_hash {
        UserOrTokenFilter::Token(th.trim())
    } else if let Some(ref gname) = rule.group_name {
        if rule.scope_mode == "shared_group" {
            let mut members = crate::db::group::get_all_group_members_for_group_name(gname)
                .await
                .unwrap_or_default();
            if !target_user.is_empty() && !members.iter().any(|m| m == target_user) {
                members.push(target_user.to_string());
            }
            shared_group_members = members;
            if shared_group_members.is_empty() {
                UserOrTokenFilter::UserList(vec![target_user])
            } else {
                let refs: Vec<&str> = shared_group_members.iter().map(|s| s.as_str()).collect();
                UserOrTokenFilter::UserList(refs)
            }
        } else {
            UserOrTokenFilter::UserList(vec![target_user])
        }
    } else if !target_user.is_empty() && target_user != "*" {
        let items: Vec<&str> = target_user
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "*")
            .collect();
        if items.is_empty() {
            UserOrTokenFilter::Any
        } else {
            UserOrTokenFilter::UserList(items)
        }
    } else if let Some(ref u) = rule.user {
        let items: Vec<&str> = u
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "*")
            .collect();
        if items.is_empty() {
            UserOrTokenFilter::Any
        } else {
            UserOrTokenFilter::UserList(items)
        }
    } else {
        UserOrTokenFilter::Any
    };

    let column_expr = match rule.unit.as_str() {
        "requests" => "COUNT(*)",
        "prompt_tokens" => "COALESCE(SUM(prompt_tokens), 0)",
        "completion_tokens" => "COALESCE(SUM(completion_tokens), 0)",
        _ => "COALESCE(SUM(total_tokens), 0)",
    };

    let used: i64 = query_request_scalar_i64(column_expr, &target_filter, &model_filter, start_time)
        .await
        .unwrap_or(0);

    let remaining = (rule.limit_value - used).max(0);

    let (reset_in_seconds, reset_timestamp) = if let Some(dur) = duration {
        let oldest_in_window: Option<i64> =
            query_request_scalar_i64("MIN(enqueue)", &target_filter, &model_filter, start_time).await;

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
        group_name: rule.group_name.clone(),
        scope_mode: rule.scope_mode.clone(),
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
