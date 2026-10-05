use crate::auth::extractor::AuthUser;
use crate::auth::oidc::{OidcClient, decode_jwt_payload_unverified, extract_claims_from_json, generate_pkce_pair, generate_random_token};
use crate::auth::session::SESSIONS;
use crate::config::CONFIG;
use crate::db::user::{create_user, get_user_by_username};
use axum::extract::Query;
use axum::http::header::{HeaderMap, SET_COOKIE};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use log::{error, info};
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::json;

static OIDC: Lazy<OidcClient> = Lazy::new(OidcClient::new);

#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    pub redirect_uri: Option<String>,
    pub json: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CallbackPayload {
    pub code: String,
    pub state: String,
}

pub fn auth_routes() -> Router {
    Router::new()
        .route("/config", get(auth_config))
        .route("/login", get(auth_login))
        .route("/callback", get(auth_callback_get).post(auth_callback_post))
        .route("/me", get(auth_me))
        .route("/logout", post(auth_logout))
}

async fn auth_config() -> Json<serde_json::Value> {
    Json(json!({
        "oidc_enabled": CONFIG.oidc_enabled,
        "issuer": CONFIG.oidc_issuer_url,
        "client_id": CONFIG.oidc_client_id,
        "scopes": CONFIG.oidc_scopes,
        "redirect_uri": CONFIG.oidc_redirect_uri,
    }))
}

async fn auth_login(
    Query(query): Query<LoginQuery>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let issuer = CONFIG.oidc_issuer_url.as_deref().unwrap_or_default();
    let discovery_url = CONFIG.oidc_discovery_url.as_deref();
    let client_id = CONFIG.oidc_client_id.as_deref().unwrap_or_default();

    if issuer.is_empty() && discovery_url.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "OIDC is not configured on this server" })),
        ));
    }

    let discovery = OIDC
        .get_discovery(issuer, discovery_url)
        .await
        .map_err(|e| {
            error!("Failed to fetch OIDC discovery: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to discover OIDC configuration: {e}") })),
            )
        })?;

    let redirect_uri = query
        .redirect_uri
        .or_else(|| CONFIG.oidc_redirect_uri.clone())
        .unwrap_or_else(|| {
            let host = headers
                .get("host")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("localhost:3000");
            let scheme = if uri.scheme_str() == Some("https") || headers.contains_key("x-forwarded-proto") {
                "https"
            } else {
                "http"
            };
            format!("{scheme}://{host}/auth/callback")
        });

    let pkce = generate_pkce_pair();
    let state = generate_random_token(24);
    let nonce = generate_random_token(24);

    SESSIONS
        .insert_pkce(
            state.clone(),
            pkce.code_verifier,
            nonce.clone(),
            redirect_uri.clone(),
        )
        .await;

    let auth_url = OidcClient::build_authorization_url(
        &discovery.authorization_endpoint,
        client_id,
        &redirect_uri,
        &CONFIG.oidc_scopes,
        &state,
        &nonce,
        &pkce.code_challenge,
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to build auth URL: {e}") })),
        )
    })?;

    let wants_json = query.json.unwrap_or(false)
        || headers
            .get("accept")
            .and_then(|a| a.to_str().ok())
            .is_some_and(|a| a.contains("application/json") && !a.contains("text/html"));

    if wants_json {
        Ok(Json(json!({
            "auth_url": auth_url,
            "state": state,
            "redirect_uri": redirect_uri,
        }))
        .into_response())
    } else {
        Ok(Redirect::temporary(&auth_url).into_response())
    }
}

#[allow(clippy::too_many_lines)]
async fn handle_callback(
    code: &str,
    state: &str,
    is_browser_get: bool,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let pending = SESSIONS.take_pkce(state).await.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid or expired state parameter" })),
        )
    })?;

    let issuer = CONFIG.oidc_issuer_url.as_deref().unwrap_or_default();
    let discovery_url = CONFIG.oidc_discovery_url.as_deref();
    let client_id = CONFIG.oidc_client_id.as_deref().unwrap_or_default();

    let discovery = OIDC
        .get_discovery(issuer, discovery_url)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("OIDC discovery failed: {e}") })),
            )
        })?;

    let token_res = OIDC
        .exchange_code(
            &discovery.token_endpoint,
            client_id,
            CONFIG.oidc_client_secret.as_ref(),
            code,
            &pending.code_verifier,
            &pending.redirect_uri,
        )
        .await
        .map_err(|e| {
            error!("Token exchange failed: {e}");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Token exchange failed: {e}") })),
            )
        })?;

    let mut claims = None;
    if let Some(ref id_token) = token_res.id_token
        && let Some(payload) = decode_jwt_payload_unverified(id_token)
    {
        claims = Some(extract_claims_from_json(&payload));
    }

    if let Some(ref userinfo_endpoint) = discovery.userinfo_endpoint
        && let Ok(u_claims) = OIDC
            .fetch_userinfo(userinfo_endpoint, &token_res.access_token)
            .await
    {
        if let Some(ref mut c) = claims {
            if c.preferred_username.is_none() {
                c.preferred_username = u_claims.preferred_username;
            }
            if c.email.is_none() {
                c.email = u_claims.email;
            }
            if c.name.is_none() {
                c.name = u_claims.name;
            }
            if let (Some(r1), Some(r2)) = (&mut c.roles, u_claims.roles) {
                r1.extend(r2);
            }
        } else {
            claims = Some(u_claims);
        }
    }

    let claims = claims.unwrap_or_default();
    let username = claims
        .preferred_username
        .or_else(|| claims.email.clone())
        .or(claims.sub)
        .unwrap_or_else(|| "oidc_user".to_string());

    let email = claims.email.clone();
    let name = claims.name.clone();
    let roles = claims.roles.unwrap_or_default();

    // Check admin status:
    // 1. Is user in DB and has is_admin == 1?
    let existing_db_user = get_user_by_username(&username).await.ok().flatten();
    let db_admin = existing_db_user.as_ref().is_some_and(|u| u.is_admin);

    // 2. Is username or email in CONFIG.oidc_admin_users?
    let config_admin = CONFIG.is_user_admin_by_config(&username)
        || email.as_deref().is_some_and(|e| CONFIG.is_user_admin_by_config(e));

    // 3. Does user have a configured admin role?
    let role_admin = roles
        .iter()
        .any(|r| CONFIG.oidc_admin_roles.contains(r) || r.to_lowercase() == "admin");

    let is_admin = db_admin || config_admin || role_admin;

    // Create or ensure user exists in database
    if existing_db_user.is_none() {
        if let Err(e) = create_user(&username, None, None, is_admin).await {
            error!("Failed to auto-create user in database: {e}");
        }
    } else if (config_admin || role_admin) && !db_admin {
        // Upgrade user to admin if role/config requires it
        let _ = create_user(
            &username,
            existing_db_user.as_ref().and_then(|u| u.limit_to_models.as_ref()),
            existing_db_user.as_ref().and_then(|u| u.log_request_body),
            true,
        )
        .await;
    }

    let ttl_secs = token_res.expires_in.unwrap_or(86400 * 7);
    let session_token = SESSIONS
        .create_session(
            username.clone(),
            email.clone(),
            name.clone(),
            roles.clone(),
            is_admin,
            ttl_secs,
        )
        .await;

    info!("User '{username}' successfully authenticated via OIDC (is_admin: {is_admin})");

    let cookie_header = format!(
        "session_token={session_token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ttl_secs}"
    );

    if is_browser_get {
        let mut response = Redirect::to("/").into_response();
        response
            .headers_mut()
            .insert(SET_COOKIE, cookie_header.parse().unwrap());
        Ok(response)
    } else {
        let mut response = Json(json!({
            "token": session_token,
            "user": {
                "username": username,
                "email": email,
                "name": name,
                "is_admin": is_admin,
                "roles": roles.into_iter().collect::<Vec<_>>(),
            },
            "expires_in": ttl_secs,
        }))
        .into_response();

        response
            .headers_mut()
            .insert(SET_COOKIE, cookie_header.parse().unwrap());
        Ok(response)
    }
}

async fn auth_callback_get(
    Query(query): Query<CallbackQuery>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    if let Some(err) = query.error {
        let desc = query.error_description.unwrap_or_default();
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("OIDC authorization error: {err} ({desc})") })),
        ));
    }

    let code = query.code.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Missing code parameter" })),
        )
    })?;

    let state = query.state.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Missing state parameter" })),
        )
    })?;

    handle_callback(&code, &state, true).await
}

async fn auth_callback_post(
    Json(payload): Json<CallbackPayload>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    handle_callback(&payload.code, &payload.state, false).await
}

async fn auth_me(user: AuthUser) -> Json<serde_json::Value> {
    Json(json!({
        "username": user.username,
        "email": user.email,
        "is_admin": user.is_admin,
        "models": user.user.as_ref().and_then(|u| u.limit_to_models.clone()),
        "log_request_body": user.user.as_ref().and_then(|u| u.log_request_body),
    }))
}

async fn auth_logout(user: AuthUser) -> Response {
    SESSIONS.remove_session(&user.token).await;
    let mut resp = Json(json!({ "status": "logged_out" })).into_response();
    resp.headers_mut().insert(
        SET_COOKIE,
        "session_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
            .parse()
            .unwrap(),
    );
    resp
}
