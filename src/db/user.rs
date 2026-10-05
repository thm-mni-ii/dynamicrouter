use crate::db::connection::get_executor;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub limit_to_models: Option<HashSet<String>>,
    pub log_request_body: Option<bool>,
    pub is_admin: bool,
}

#[derive(sqlx::FromRow)]
struct UserRow {
    username: String,
    limit_to_models: Option<String>,
    log_request_body: Option<bool>,
    is_admin: Option<bool>,
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
        }
    }
}

pub async fn get_user_by_token_hash(token_hash: &str) -> Result<Option<User>, anyhow::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        r"
        SELECT u.username, u.limit_to_models, u.log_request_body, u.is_admin
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

pub async fn get_user_by_username(username: &str) -> Result<Option<User>, anyhow::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        r"
        SELECT username, limit_to_models, log_request_body, is_admin
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
        SELECT username, limit_to_models, log_request_body, is_admin
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
) -> Result<(), anyhow::Error> {
    let limit_str = limit_to_models.map(|l| serde_json::to_string(l).unwrap_or_default());
    sqlx::query(
        r"
        INSERT INTO users (username, limit_to_models, log_request_body, is_admin)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(username) DO UPDATE SET
            limit_to_models = excluded.limit_to_models,
            log_request_body = excluded.log_request_body,
            is_admin = excluded.is_admin
        ",
    )
    .bind(username)
    .bind(limit_str)
    .bind(log_request_body)
    .bind(is_admin)
    .execute(get_executor())
    .await?;
    Ok(())
}

pub async fn delete_user(username: &str) -> Result<bool, anyhow::Error> {
    let res = sqlx::query("DELETE FROM users WHERE username = ?")
        .bind(username)
        .execute(get_executor())
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn add_user_token(token_hash: &str, username: &str) -> Result<(), anyhow::Error> {
    sqlx::query(
        r"
        INSERT INTO user_tokens (token_hash, username)
        VALUES (?, ?)
        ON CONFLICT(token_hash) DO UPDATE SET username = excluded.username
        ",
    )
    .bind(token_hash)
    .bind(username)
    .execute(get_executor())
    .await?;
    Ok(())
}

pub async fn get_user_tokens(username: &str) -> Result<Vec<String>, anyhow::Error> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT token_hash FROM user_tokens WHERE username = ? ORDER BY token_hash",
    )
    .bind(username)
    .fetch_all(get_executor())
    .await?;

    Ok(rows)
}

pub fn hash_token(raw_token: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(raw_token.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn generate_and_add_user_token(username: &str) -> Result<String, anyhow::Error> {
    let mut random_bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut random_bytes);
    let raw_token = format!("sk-dr-{}", hex::encode(random_bytes));
    let hash = hash_token(&raw_token);

    add_user_token(&hash, username).await?;
    Ok(raw_token)
}

pub async fn delete_user_token(token_hash: &str) -> Result<bool, anyhow::Error> {
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
    let res = sqlx::query("DELETE FROM user_tokens WHERE username = ? AND token_hash = ?")
        .bind(username)
        .bind(token_hash)
        .execute(get_executor())
        .await?;
    Ok(res.rows_affected() > 0)
}
