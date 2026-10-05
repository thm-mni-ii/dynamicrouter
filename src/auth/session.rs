use crate::auth::oidc::generate_random_token;
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
        let session_token = format!("dr-sess-{}", generate_random_token(32));
        let session = AuthSessionData {
            session_token: session_token.clone(),
            username,
            email,
            name,
            roles,
            is_admin,
            created_at: now,
            expires_at: now + ttl_secs,
        };

        let mut map = self.sessions_map.write().await;
        // Cleanup expired sessions
        map.retain(|_, v| v.expires_at > now);
        map.insert(session_token.clone(), session);
        session_token
    }

    pub async fn get_session(&self, token: &str) -> Option<AuthSessionData> {
        let now = Utc::now().timestamp();
        let map = self.sessions_map.read().await;
        let session = map.get(token)?;
        if session.expires_at <= now {
            return None;
        }
        Some(session.clone())
    }

    pub async fn remove_session(&self, token: &str) -> bool {
        let mut map = self.sessions_map.write().await;
        map.remove(token).is_some()
    }
}

pub static SESSIONS: Lazy<Arc<SessionStore>> = Lazy::new(|| Arc::new(SessionStore::new()));
