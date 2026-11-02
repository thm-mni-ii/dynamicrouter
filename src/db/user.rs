use crate::db::connection::get_executor;
use crate::db::rate_limit::{
    CreateRateLimitPayload, RateLimitRule, create_rate_limit, delete_rate_limits_for_token,
    delete_rate_limits_for_user, get_rate_limits_for_token,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub username: String,
    pub limit_to_models: Option<HashSet<String>>,
    pub log_request_body: Option<bool>,
    pub is_admin: bool,
    pub expires_at: Option<i64>,
}

impl User {
    #[must_use]
    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            chrono::Utc::now().timestamp() > exp
        } else {
            false
        }
    }
}

#[derive(sqlx::FromRow)]
struct UserRow {
    username: String,
    limit_to_models: Option<String>,
    log_request_body: Option<bool>,
    is_admin: Option<bool>,
    expires_at: Option<i64>,
}

impl UserRow {
    fn into_user(self) -> User {
        let limit_to_models = self
            .limit_to_models
            .and_then(|s| serde_json::from_str::<HashSet<String>>(&s).ok());
        User {
            username: self.username,
            limit_to_models,
            log_request_body: self.log_request_body,
            is_admin: self.is_admin.unwrap_or(false),
            expires_at: self.expires_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserToken {
    pub token_hash: String,
    pub username: String,
    pub name: Option<String>,
    pub token_prefix: Option<String>,
    pub token_suffix: Option<String>,
    pub limit_to_models: Option<HashSet<String>>,
    pub expires_at: Option<i64>,
    pub allowed_ips: Option<Vec<String>>,
    pub rate_limits: Vec<RateLimitRule>,
    pub created_at: Option<i64>,
}

impl UserToken {
    #[must_use]
    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            chrono::Utc::now().timestamp() > exp
        } else {
            false
        }
    }

    #[must_use]
    pub fn is_ip_allowed(&self, client_ip: Option<std::net::IpAddr>) -> bool {
        let Some(ref allowed) = self.allowed_ips else {
            return true;
        };
        if allowed.is_empty() {
            return true;
        }
        let Some(ip) = client_ip else {
            return false;
        };
        for entry in allowed {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            if let Ok(net) = entry.parse::<ipnet::IpNet>() {
                if net.contains(&ip) {
                    return true;
                }
            } else if let Ok(single_ip) = entry.parse::<std::net::IpAddr>() {
                if single_ip == ip {
                    return true;
                }
            }
        }
        false
    }

    #[must_use]
    pub fn preview_display(&self) -> String {
        match (&self.token_prefix, &self.token_suffix) {
            (Some(p), Some(s)) => format!("sk-dr-v1-{p}...{s}"),
            _ => {
                if self.token_hash.len() >= 8 {
                    format!("Legacy Token ({}...)", &self.token_hash[..8])
                } else {
                    "Legacy Token".to_string()
                }
            }
        }
    }
}

#[derive(sqlx::FromRow)]
struct UserTokenRow {
    token_hash: String,
    username: String,
    name: Option<String>,
    token_prefix: Option<String>,
    token_suffix: Option<String>,
    limit_to_models: Option<String>,
    expires_at: Option<i64>,
    allowed_ips: Option<String>,
    created_at: Option<i64>,
}

impl UserTokenRow {
    fn into_user_token(self, rate_limits: Vec<RateLimitRule>) -> UserToken {
        let limit_to_models = self
            .limit_to_models
            .and_then(|s| serde_json::from_str::<HashSet<String>>(&s).ok());
        let allowed_ips = self
            .allowed_ips
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok());

        UserToken {
            token_hash: self.token_hash,
            username: self.username,
            name: self.name,
            token_prefix: self.token_prefix,
            token_suffix: self.token_suffix,
            limit_to_models,
            expires_at: self.expires_at,
            allowed_ips,
            rate_limits,
            created_at: self.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenRateLimitInput {
    pub model: Option<String>,
    pub time_window: String,
    pub limit_value: i64,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateTokenInput {
    pub name: Option<String>,
    pub limit_to_models: Option<HashSet<String>>,
    pub expires_at: Option<i64>,
    pub allowed_ips: Option<Vec<String>>,
    pub rate_limits: Option<Vec<TokenRateLimitInput>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateTokenInput {
    pub name: Option<String>,
    pub limit_to_models: Option<HashSet<String>>,
    pub expires_at: Option<i64>,
    pub allowed_ips: Option<Vec<String>>,
    pub rate_limits: Option<Vec<TokenRateLimitInput>>,
}

#[must_use]
pub fn normalize_token_secret(token: &str) -> &str {
    token
        .strip_prefix("sk-dr-v1-")
        .or_else(|| token.strip_prefix("sk-dr-v1"))
        .unwrap_or(token)
}

pub fn hash_token(raw_token: &str) -> String {
    let normalized = normalize_token_secret(raw_token);
    let mut hasher = sha2::Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn get_user_by_token_hash(token_hash: &str) -> Result<Option<User>, anyhow::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        r"
        SELECT u.username, u.limit_to_models, u.log_request_body, u.is_admin, u.expires_at
        FROM user_tokens ut
        JOIN users u ON ut.username = u.username
        WHERE ut.token_hash = ?
        ",
    )
    .bind(token_hash)
    .fetch_optional(get_executor())
    .await?;

    Ok(row.map(UserRow::into_user))
}

pub async fn get_user_and_token_by_token_hash(
    token_hash: &str,
) -> Result<Option<(User, UserToken)>, anyhow::Error> {
    let user_row = sqlx::query_as::<_, UserRow>(
        r"
        SELECT u.username, u.limit_to_models, u.log_request_body, u.is_admin, u.expires_at
        FROM user_tokens ut
        JOIN users u ON ut.username = u.username
        WHERE ut.token_hash = ?
        ",
    )
    .bind(token_hash)
    .fetch_optional(get_executor())
    .await?;

    let Some(user_row) = user_row else {
        return Ok(None);
    };

    let token_row = sqlx::query_as::<_, UserTokenRow>(
        r"
        SELECT token_hash, username, name, token_prefix, token_suffix, limit_to_models, expires_at, allowed_ips, created_at
        FROM user_tokens
        WHERE token_hash = ?
        ",
    )
    .bind(token_hash)
    .fetch_optional(get_executor())
    .await?;

    let Some(token_row) = token_row else {
        return Ok(None);
    };

    let rate_limits = get_rate_limits_for_token(token_hash).await.unwrap_or_default();
    let user = user_row.into_user();
    let token = token_row.into_user_token(rate_limits);

    Ok(Some((user, token)))
}

pub async fn get_user_by_username(username: &str) -> Result<Option<User>, anyhow::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        r"
        SELECT username, limit_to_models, log_request_body, is_admin, expires_at
        FROM users
        WHERE username = ?
        ",
    )
    .bind(username)
    .fetch_optional(get_executor())
    .await?;

    Ok(row.map(UserRow::into_user))
}

pub async fn get_all_users() -> Result<Vec<User>, anyhow::Error> {
    let rows = sqlx::query_as::<_, UserRow>(
        r"
        SELECT username, limit_to_models, log_request_body, is_admin, expires_at
        FROM users
        ORDER BY username
        ",
    )
    .fetch_all(get_executor())
    .await?;

    Ok(rows.into_iter().map(UserRow::into_user).collect())
}

pub async fn create_user(
    username: &str,
    limit_to_models: Option<&HashSet<String>>,
    log_request_body: Option<bool>,
    is_admin: bool,
    expires_at: Option<i64>,
) -> Result<(), anyhow::Error> {
    let limit_str = limit_to_models.map(|l| serde_json::to_string(l).unwrap_or_default());
    sqlx::query(
        r"
        INSERT INTO users (username, limit_to_models, log_request_body, is_admin, expires_at)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(username) DO UPDATE SET
            limit_to_models = excluded.limit_to_models,
            log_request_body = excluded.log_request_body,
            is_admin = excluded.is_admin,
            expires_at = excluded.expires_at
        ",
    )
    .bind(username)
    .bind(limit_str)
    .bind(log_request_body)
    .bind(is_admin)
    .bind(expires_at)
    .execute(get_executor())
    .await?;
    Ok(())
}

pub async fn delete_user(username: &str) -> Result<bool, anyhow::Error> {
    delete_rate_limits_for_user(username).await.ok();
    let res = sqlx::query("DELETE FROM users WHERE username = ?")
        .bind(username)
        .execute(get_executor())
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn add_user_token(token_hash: &str, username: &str) -> Result<(), anyhow::Error> {
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        r"
        INSERT INTO user_tokens (token_hash, username, created_at)
        VALUES (?, ?, ?)
        ON CONFLICT(token_hash) DO UPDATE SET username = excluded.username
        ",
    )
    .bind(token_hash)
    .bind(username)
    .bind(now)
    .execute(get_executor())
    .await?;
    Ok(())
}

pub async fn get_user_tokens(username: &str) -> Result<Vec<UserToken>, anyhow::Error> {
    let rows = sqlx::query_as::<_, UserTokenRow>(
        r"
        SELECT token_hash, username, name, token_prefix, token_suffix, limit_to_models, expires_at, allowed_ips, created_at
        FROM user_tokens
        WHERE username = ?
        ORDER BY created_at DESC, token_hash ASC
        ",
    )
    .bind(username)
    .fetch_all(get_executor())
    .await?;

    let mut result = Vec::new();
    for row in rows {
        let rate_limits = get_rate_limits_for_token(&row.token_hash).await.unwrap_or_default();
        result.push(row.into_user_token(rate_limits));
    }

    Ok(result)
}

pub async fn generate_and_add_user_token(
    username: &str,
    input: Option<CreateTokenInput>,
) -> Result<(String, UserToken), anyhow::Error> {
    let mut random_bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut random_bytes);
    let secret_hex = hex::encode(random_bytes);
    let raw_token = format!("sk-dr-v1-{secret_hex}");
    let hash = hash_token(&secret_hex);

    let prefix = secret_hex[0..3].to_string();
    let suffix = secret_hex[secret_hex.len() - 3..].to_string();

    let input = input.unwrap_or_default();
    let limit_str = input
        .limit_to_models
        .as_ref()
        .map(|l| serde_json::to_string(l).unwrap_or_default());
    let ips_str = input
        .allowed_ips
        .as_ref()
        .map(|l| serde_json::to_string(l).unwrap_or_default());
    let now = chrono::Utc::now().timestamp();

    let trimmed_name = input.name.as_deref().and_then(|n| {
        let t = n.trim();
        if t.is_empty() { None } else { Some(t.to_string()) }
    });

    sqlx::query(
        r"
        INSERT INTO user_tokens (token_hash, username, name, token_prefix, token_suffix, limit_to_models, expires_at, allowed_ips, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(token_hash) DO UPDATE SET
            username = excluded.username,
            name = excluded.name,
            token_prefix = excluded.token_prefix,
            token_suffix = excluded.token_suffix,
            limit_to_models = excluded.limit_to_models,
            expires_at = excluded.expires_at,
            allowed_ips = excluded.allowed_ips
        ",
    )
    .bind(&hash)
    .bind(username)
    .bind(&trimmed_name)
    .bind(&prefix)
    .bind(&suffix)
    .bind(limit_str)
    .bind(input.expires_at)
    .bind(ips_str)
    .bind(now)
    .execute(get_executor())
    .await?;

    let mut created_rules = Vec::new();
    if let Some(rules) = input.rate_limits {
        for r in rules {
            if r.limit_value > 0 {
                let created_rule = create_rate_limit(&CreateRateLimitPayload {
                    user: Some(username.to_string()),
                    token_hash: Some(hash.clone()),
                    group_name: None,
                    scope_mode: None,
                    model: r.model,
                    time_window: r.time_window,
                    limit_value: r.limit_value,
                    unit: r.unit,
                })
                .await?;
                created_rules.push(created_rule);
            }
        }
    }

    let user_token = UserToken {
        token_hash: hash,
        username: username.to_string(),
        name: trimmed_name,
        token_prefix: Some(prefix),
        token_suffix: Some(suffix),
        limit_to_models: input.limit_to_models,
        expires_at: input.expires_at,
        allowed_ips: input.allowed_ips,
        rate_limits: created_rules,
        created_at: Some(now),
    };

    Ok((raw_token, user_token))
}

pub async fn update_user_token_for_user(
    username: &str,
    token_hash: &str,
    input: UpdateTokenInput,
) -> Result<Option<UserToken>, anyhow::Error> {
    // Verify existence and ownership
    let existing = sqlx::query_as::<_, UserTokenRow>(
        "SELECT token_hash, username, name, token_prefix, token_suffix, limit_to_models, expires_at, allowed_ips, created_at FROM user_tokens WHERE token_hash = ? AND username = ?",
    )
    .bind(token_hash)
    .bind(username)
    .fetch_optional(get_executor())
    .await?;

    let Some(existing_row) = existing else {
        return Ok(None);
    };

    let trimmed_name = input.name.as_deref().and_then(|n| {
        let t = n.trim();
        if t.is_empty() { None } else { Some(t.to_string()) }
    });
    let limit_str = input
        .limit_to_models
        .as_ref()
        .map(|l| serde_json::to_string(l).unwrap_or_default());
    let ips_str = input
        .allowed_ips
        .as_ref()
        .map(|l| serde_json::to_string(l).unwrap_or_default());

    sqlx::query(
        r"
        UPDATE user_tokens
        SET name = ?, limit_to_models = ?, expires_at = ?, allowed_ips = ?
        WHERE token_hash = ? AND username = ?
        ",
    )
    .bind(&trimmed_name)
    .bind(limit_str)
    .bind(input.expires_at)
    .bind(ips_str)
    .bind(token_hash)
    .bind(username)
    .execute(get_executor())
    .await?;

    if let Some(rules) = input.rate_limits {
        delete_rate_limits_for_token(token_hash).await?;
        for r in rules {
            if r.limit_value > 0 {
                create_rate_limit(&CreateRateLimitPayload {
                    user: Some(username.to_string()),
                    token_hash: Some(token_hash.to_string()),
                    group_name: None,
                    scope_mode: None,
                    model: r.model,
                    time_window: r.time_window,
                    limit_value: r.limit_value,
                    unit: r.unit,
                })
                .await?;
            }
        }
    }

    let updated_rules = get_rate_limits_for_token(token_hash).await.unwrap_or_default();
    Ok(Some(UserToken {
        token_hash: token_hash.to_string(),
        username: username.to_string(),
        name: trimmed_name,
        token_prefix: existing_row.token_prefix,
        token_suffix: existing_row.token_suffix,
        limit_to_models: input.limit_to_models,
        expires_at: input.expires_at,
        allowed_ips: input.allowed_ips,
        rate_limits: updated_rules,
        created_at: existing_row.created_at,
    }))
}

pub async fn delete_user_token(token_hash: &str) -> Result<bool, anyhow::Error> {
    delete_rate_limits_for_token(token_hash).await.ok();
    let res = sqlx::query("DELETE FROM user_tokens WHERE token_hash = ?")
        .bind(token_hash)
        .execute(get_executor())
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn delete_user_token_for_user(
    username: &str,
    token_hash: &str,
) -> Result<bool, anyhow::Error> {
    delete_rate_limits_for_token(token_hash).await.ok();
    let res = sqlx::query("DELETE FROM user_tokens WHERE username = ? AND token_hash = ?")
        .bind(username)
        .bind(token_hash)
        .execute(get_executor())
        .await?;
    Ok(res.rows_affected() > 0)
}
