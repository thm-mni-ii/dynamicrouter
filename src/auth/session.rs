use crate::auth::oidc::generate_random_token;
use crate::config::CONFIG;
use chrono::Utc;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PkcePending {
    pub state: String,
    pub code_verifier: String,
    pub nonce: String,
    pub redirect_uri: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSessionData {
    pub session_token: String,
    pub username: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub roles: HashSet<String>,
    pub is_admin: bool,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    session_token: String,
    username: String,
    email: Option<String>,
    name: Option<String>,
    roles: String,
    is_admin: bool,
    created_at: i64,
    expires_at: i64,
}

pub struct SessionStore {
    pkce_map: RwLock<HashMap<String, PkcePending>>,
    sessions_map: RwLock<HashMap<String, AuthSessionData>>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    pub fn new() -> Self {
        SessionStore {
            pkce_map: RwLock::new(HashMap::new()),
            sessions_map: RwLock::new(HashMap::new()),
        }
    }

    pub async fn insert_pkce(
        &self,
        state: String,
        code_verifier: String,
        nonce: String,
        redirect_uri: String,
    ) {
        let now = Utc::now().timestamp();
        let mut map = self.pkce_map.write().await;
        // Clean up expired PKCE requests (older than 10 mins)
        map.retain(|_, v| now - v.created_at < 600);
        map.insert(
            state.clone(),
            PkcePending {
                state,
                code_verifier,
                nonce,
                redirect_uri,
                created_at: now,
            },
        );
    }

    pub async fn take_pkce(&self, state: &str) -> Option<PkcePending> {
        let now = Utc::now().timestamp();
        let mut map = self.pkce_map.write().await;
        let pending = map.remove(state)?;
        if now - pending.created_at > 600 {
            return None;
        }
        Some(pending)
    }

    pub async fn create_session(
        &self,
        username: String,
        email: Option<String>,
        name: Option<String>,
        roles: HashSet<String>,
        is_admin: bool,
        ttl_secs: i64,
    ) -> String {
        let now = Utc::now().timestamp();
        let ttl = if ttl_secs > 0 { ttl_secs } else { CONFIG.session_ttl_secs };
        let session_token = format!("dr-sess-{}", generate_random_token(32));
        let session = AuthSessionData {
            session_token: session_token.clone(),
            username: username.clone(),
            email: email.clone(),
            name: name.clone(),
            roles: roles.clone(),
            is_admin,
            created_at: now,
            expires_at: now + ttl,
        };

        // 1. Update in-memory session cache
        {
            let mut map = self.sessions_map.write().await;
            map.retain(|_, v| v.expires_at > now);
            map.insert(session_token.clone(), session.clone());
        }

        // 2. Persist to SQLite database if initialized
        if crate::db::connection::DB.get().is_some() {
            let pool = crate::db::connection::get_executor();
            let roles_json = serde_json::to_string(&roles).unwrap_or_else(|_| "[]".to_string());
            let _ = sqlx::query(
                "INSERT OR REPLACE INTO sessions (session_token, username, email, name, roles, is_admin, created_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&session_token)
            .bind(&username)
            .bind(&email)
            .bind(&name)
            .bind(&roles_json)
            .bind(is_admin)
            .bind(now)
            .bind(now + ttl)
            .execute(pool)
            .await;
        }

        session_token
    }

    pub async fn get_session(&self, token: &str) -> Option<AuthSessionData> {
        let now = Utc::now().timestamp();
        let ttl = CONFIG.session_ttl_secs;

        // 1. Check in-memory session cache first
        let session_opt = {
            let map = self.sessions_map.read().await;
            map.get(token).cloned()
        };

        let mut session = match session_opt {
            Some(s) => {
                if s.expires_at <= now {
                    return None;
                }
                s
            }
            None => {
                // 2. Try loading from SQLite database on cache miss (e.g. after server restart)
                if crate::db::connection::DB.get().is_some() {
                    let pool = crate::db::connection::get_executor();
                    let row = sqlx::query_as::<_, SessionRow>(
                        "SELECT session_token, username, email, name, roles, is_admin, created_at, expires_at FROM sessions WHERE session_token = ? AND expires_at > ?"
                    )
                    .bind(token)
                    .bind(now)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();

                    if let Some(r) = row {
                        let roles = serde_json::from_str::<HashSet<String>>(&r.roles).unwrap_or_default();
                        let s = AuthSessionData {
                            session_token: r.session_token,
                            username: r.username,
                            email: r.email,
                            name: r.name,
                            roles,
                            is_admin: r.is_admin,
                            created_at: r.created_at,
                            expires_at: r.expires_at,
                        };
                        let mut map = self.sessions_map.write().await;
                        map.insert(token.to_string(), s.clone());
                        s
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
        };

        // 3. Sliding expiration: if remaining lifetime is less than half the TTL, extend it
        if session.expires_at - now < (ttl / 2).max(60) {
            let new_expires_at = now + ttl;
            session.expires_at = new_expires_at;

            {
                let mut map = self.sessions_map.write().await;
                if let Some(entry) = map.get_mut(token) {
                    entry.expires_at = new_expires_at;
                }
            }

            if crate::db::connection::DB.get().is_some() {
                let pool = crate::db::connection::get_executor();
                let _ = sqlx::query("UPDATE sessions SET expires_at = ? WHERE session_token = ?")
                    .bind(new_expires_at)
                    .bind(token)
                    .execute(pool)
                    .await;
            }
        }

        Some(session)
    }

    pub async fn remove_session(&self, token: &str) -> bool {
        let mut map = self.sessions_map.write().await;
        let removed = map.remove(token).is_some();

        if crate::db::connection::DB.get().is_some() {
            let pool = crate::db::connection::get_executor();
            let _ = sqlx::query("DELETE FROM sessions WHERE session_token = ?")
                .bind(token)
                .execute(pool)
                .await;
        }

        removed
    }
}

pub static SESSIONS: Lazy<Arc<SessionStore>> = Lazy::new(|| Arc::new(SessionStore::new()));
