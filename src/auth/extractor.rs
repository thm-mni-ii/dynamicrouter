use crate::auth::oidc::{decode_jwt_payload_unverified, extract_claims_from_json};
use crate::auth::session::SESSIONS;
use crate::config::CONFIG;
use crate::db::user::{User, get_user_by_token_hash, get_user_by_username, hash_token};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub username: String,
    pub email: Option<String>,
    pub is_admin: bool,
    pub user: Option<User>,
    pub token: String,
}

#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthUser);

impl std::ops::Deref for AdminUser {
    type Target = AuthUser;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub enum AuthError {
    MissingToken,
    InvalidToken,
    Forbidden(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "Missing authorization token".to_string(),
            ),
            AuthError::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "Invalid or expired token".to_string(),
            ),
            AuthError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

fn extract_token_from_parts(parts: &Parts) -> Option<String> {
    if let Some(auth_header) = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            return Some(token.trim().to_string());
        }
        return Some(auth_header.trim().to_string());
    }

    if let Some(cookie_header) = parts
        .headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
    {
        for cookie in cookie_header.split(';') {
            let cookie = cookie.trim();
            if let Some(val) = cookie.strip_prefix("session_token=") {
                return Some(val.to_string());
            }
            if let Some(val) = cookie.strip_prefix("token=") {
                return Some(val.to_string());
            }
        }
    }

    None
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Some(raw_token) = extract_token_from_parts(parts) else {
            return Err(AuthError::MissingToken);
        };

        if raw_token.trim().is_empty() {
            return Err(AuthError::MissingToken);
        }

        // 1. Check in active session store (OIDC PKCE login session)
        if let Some(session) = SESSIONS.get_session(&raw_token).await {
            let db_user = get_user_by_username(&session.username)
                .await
                .ok()
                .flatten();

            let is_admin = session.is_admin
                || db_user.as_ref().is_some_and(|u| u.is_admin)
                || CONFIG.is_user_admin_by_config(&session.username)
                || session
                    .email
                    .as_deref()
                    .is_some_and(|e| CONFIG.is_user_admin_by_config(e));

            return Ok(AuthUser {
                username: session.username,
                email: session.email,
                is_admin,
                user: db_user,
                token: raw_token,
            });
        }

        // 2. Check in database user_tokens (hash of bearer token)
        let token_hash = hash_token(&raw_token);
        if let Ok(Some(user)) = get_user_by_token_hash(&token_hash).await {
            let is_admin = user.is_admin || CONFIG.is_user_admin_by_config(&user.username);
            return Ok(AuthUser {
                username: user.username.clone(),
                email: None,
                is_admin,
                user: Some(user),
                token: raw_token,
            });
        }

        // 3. Also check if the raw token was itself stored as the token hash directly
        if let Ok(Some(user)) = get_user_by_token_hash(&raw_token).await {
            let is_admin = user.is_admin || CONFIG.is_user_admin_by_config(&user.username);
            return Ok(AuthUser {
                username: user.username.clone(),
                email: None,
                is_admin,
                user: Some(user),
                token: raw_token,
            });
        }

        // 4. Try parsing as OIDC JWT token
        if let Some(payload) = decode_jwt_payload_unverified(&raw_token) {
            let claims = extract_claims_from_json(&payload);
            let username = claims
                .preferred_username
                .or(claims.email.clone())
                .or(claims.sub)
                .unwrap_or_else(|| "oidc_user".to_string());

            let db_user = get_user_by_username(&username).await.ok().flatten();

            let has_admin_role = claims.roles.as_ref().is_some_and(|roles| {
                roles
                    .iter()
                    .any(|r| CONFIG.oidc_admin_roles.contains(r) || r.to_lowercase() == "admin")
            });

            let is_admin = has_admin_role
                || db_user.as_ref().is_some_and(|u| u.is_admin)
                || CONFIG.is_user_admin_by_config(&username)
                || claims
                    .email
                    .as_deref()
                    .is_some_and(|e| CONFIG.is_user_admin_by_config(e));

            return Ok(AuthUser {
                username,
                email: claims.email,
                is_admin,
                user: db_user,
                token: raw_token,
            });
        }

        Err(AuthError::InvalidToken)
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AdminUser {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;
        if auth_user.is_admin {
            Ok(AdminUser(auth_user))
        } else {
            Err(AuthError::Forbidden(
                "Administrator privileges required to access this resource".to_string(),
            ))
        }
    }
}
