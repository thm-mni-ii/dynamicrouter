use crate::auth::extractor::AuthUser;
use crate::auth::oidc::{
    decode_jwt_payload_unverified, extract_claims_from_json, generate_pkce_pair,
    generate_random_token, OidcClient,
};
use crate::auth::session::SESSIONS;
use crate::config::CONFIG;
use crate::db::user::{create_user, get_user_by_username};
use crate::ui::CSP_VALUE;
use axum::extract::Query;
use axum::http::header::{HeaderMap, HeaderValue, CONTENT_SECURITY_POLICY, CONTENT_TYPE, SET_COOKIE, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS, REFERRER_POLICY};
use axum::http::{StatusCode, Uri};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use log::{error, info, warn};
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
        "oidc_auto_register": CONFIG.oidc_auto_register,
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

fn html_response(status: StatusCode, html: String) -> Response {
    let mut response = (status, Html(html)).into_response();
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    headers.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP_VALUE));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("strict-origin-when-cross-origin"));
    response
}

fn render_callback_success_html(username: &str, is_admin: bool, session_token: &str) -> String {
    let role_badge = if is_admin {
        r#"<span class="badge badge-admin">Administrator</span>"#
    } else {
        r#"<span class="badge badge-user">Standard User</span>"#
    };

    format!(r#"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="{CSP_VALUE}">
  <title>Authenticating - DynamicRouter</title>
  <style>
    @font-face {{
      font-family: 'Inter';
      font-style: normal;
      font-weight: 100 900;
      font-display: swap;
      src: url('/fonts/inter.woff2') format('woff2');
    }}
    @font-face {{
      font-family: 'Outfit';
      font-style: normal;
      font-weight: 100 900;
      font-display: swap;
      src: url('/fonts/outfit.woff2') format('woff2');
    }}
    :root {{
      --bg-base: #060913;
      --text-main: #f8fafc;
      --text-muted: #94a3b8;
      
      /* Initial White Lighting Glow */
      --glow-primary: #ffffff;
      --glow-rgb: 255, 255, 255;
      --glow-accent: #e2e8f0;
      --glow-gradient: linear-gradient(135deg, #ffffff 0%, #cbd5e1 100%);
      --glow-shadow: 0 0 35px -5px rgba(255, 255, 255, 0.35);
      
      --glass-bg: rgba(15, 23, 42, 0.7);
      --glass-border: rgba(255, 255, 255, 0.12);
    }}
    
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    
    body {{
      min-height: 100vh;
      font-family: 'Inter', system-ui, sans-serif;
      background-color: var(--bg-base);
      background-image: radial-gradient(circle at 50% 30%, #111827 0%, #060913 75%);
      color: var(--text-main);
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 1.5rem;
      overflow: hidden;
      position: relative;
    }}
    
    .ambient-glow {{
      position: fixed;
      width: 550px;
      height: 550px;
      border-radius: 50%;
      filter: blur(120px);
      opacity: 0.22;
      background: var(--glow-primary);
      transition: background 800ms cubic-bezier(0.16, 1, 0.3, 1), transform 800ms ease;
      pointer-events: none;
      z-index: 0;
    }}
    
    .glass-card {{
      position: relative;
      z-index: 1;
      width: 100%;
      max-width: 480px;
      background: var(--glass-bg);
      backdrop-filter: blur(24px);
      -webkit-backdrop-filter: blur(24px);
      border: 1px solid var(--glass-border);
      border-radius: 24px;
      padding: 2.5rem 2rem;
      text-align: center;
      box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6), var(--glow-shadow);
      transition: all 600ms cubic-bezier(0.16, 1, 0.3, 1);
    }}
    
    .status-icon-wrapper {{
      width: 64px;
      height: 64px;
      margin: 0 auto 1.5rem;
      border-radius: 20px;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 2rem;
      background: rgba(255, 255, 255, 0.08);
      border: 1px solid rgba(255, 255, 255, 0.15);
      transition: all 600ms ease;
    }}
    
    .spinner {{
      width: 32px;
      height: 32px;
      border: 3px solid rgba(255, 255, 255, 0.2);
      border-top-color: #ffffff;
      border-radius: 50%;
      animation: spin 800ms linear infinite;
    }}
    
    @keyframes spin {{
      to {{ transform: rotate(360deg); }}
    }}
    
    .title {{
      font-family: 'Outfit', sans-serif;
      font-size: 1.5rem;
      font-weight: 700;
      margin-bottom: 0.5rem;
      color: var(--text-main);
    }}
    
    .subtitle {{
      font-size: 0.95rem;
      color: var(--text-muted);
      line-height: 1.5;
    }}
    
    .badge {{
      display: inline-block;
      margin-top: 0.75rem;
      padding: 0.25rem 0.85rem;
      border-radius: 9999px;
      font-size: 0.75rem;
      font-weight: 600;
      letter-spacing: 0.04em;
    }}
    
    .badge-admin {{
      background: rgba(16, 185, 129, 0.15);
      border: 1px solid rgba(16, 185, 129, 0.4);
      color: #34d399;
    }}
    
    .badge-user {{
      background: rgba(255, 255, 255, 0.08);
      border: 1px solid rgba(255, 255, 255, 0.2);
      color: #f8fafc;
    }}
    
    .progress-bar {{
      margin-top: 2rem;
      width: 100%;
      height: 4px;
      background: rgba(255, 255, 255, 0.08);
      border-radius: 9999px;
      overflow: hidden;
      position: relative;
    }}
    
    .progress-fill {{
      height: 100%;
      width: 0%;
      background: var(--glow-gradient);
      border-radius: 9999px;
      transition: width 2500ms linear;
    }}
    
    .redirect-hint {{
      margin-top: 0.75rem;
      font-size: 0.8rem;
      color: var(--text-muted);
    }}
    
    .manual-btn {{
      margin-top: 1.25rem;
      display: inline-block;
      padding: 0.5rem 1.25rem;
      border-radius: 10px;
      font-size: 0.85rem;
      font-weight: 600;
      color: #040810;
      background: var(--glow-gradient);
      border: none;
      cursor: pointer;
      text-decoration: none;
      box-shadow: var(--glow-shadow);
      transition: transform 150ms ease;
    }}
    
    .manual-btn:hover {{
      transform: translateY(-1px);
    }}
  </style>
</head>
<body>
  <div class="ambient-glow" id="glow"></div>
  
  <div class="glass-card" id="card">
    <div class="status-icon-wrapper" id="iconBox">
      <div class="spinner" id="spinner"></div>
    </div>
    <h2 class="title" id="title">Authenticating session...</h2>
    <p class="subtitle" id="subtitle">Verifying security token and permissions...</p>
    <div id="badgeContainer"></div>
    
    <div class="progress-bar">
      <div class="progress-fill" id="progress"></div>
    </div>
    <p class="redirect-hint" id="redirectHint">Processing authentication...</p>
  </div>

  <script>
    (function() {{
      const token = "{session_token}";
      const username = "{username}";
      
      // Store session token in localStorage for frontend client
      try {{
        localStorage.setItem('dr_api_key', token);
      }} catch (e) {{}}

      // Step 1: Initial White background glow light while processing
      // Step 2: Transition to vibrant green light with success message
      setTimeout(function() {{
        // Switch variables to Emerald Green
        document.documentElement.style.setProperty('--glow-primary', '#10b981');
        document.documentElement.style.setProperty('--glow-rgb', '16, 185, 129');
        document.documentElement.style.setProperty('--glow-accent', '#34d399');
        document.documentElement.style.setProperty('--glow-gradient', 'linear-gradient(135deg, #10b981 0%, #34d399 100%)');
        document.documentElement.style.setProperty('--glow-shadow', '0 0 35px -5px rgba(16, 185, 129, 0.45)');
        document.documentElement.style.setProperty('--glass-border', 'rgba(16, 185, 129, 0.3)');
        
        // Update UI
        const iconBox = document.getElementById('iconBox');
        iconBox.innerHTML = '✓';
        iconBox.style.color = '#34d399';
        iconBox.style.background = 'rgba(16, 185, 129, 0.15)';
        iconBox.style.borderColor = 'rgba(16, 185, 129, 0.4)';
        
        document.getElementById('title').textContent = 'Login was successful as ' + username;
        document.getElementById('subtitle').textContent = 'Your identity has been verified. Welcome to DynamicRouter!';
        document.getElementById('badgeContainer').innerHTML = '{role_badge}';
        document.getElementById('redirectHint').textContent = 'Redirecting to application in 3 seconds...';
        
        const progress = document.getElementById('progress');
        progress.style.width = '100%';
        
        // Redirect after 3 seconds
        setTimeout(function() {{
          window.location.replace('/');
        }}, 2800);
      }}, 600);
    }})();
  </script>
</body>
</html>"#)
}

fn render_callback_error_html(error_message: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="{CSP_VALUE}">
  <title>Authentication Failed - DynamicRouter</title>
  <style>
    @font-face {{
      font-family: 'Inter';
      font-style: normal;
      font-weight: 100 900;
      font-display: swap;
      src: url('/fonts/inter.woff2') format('woff2');
    }}
    @font-face {{
      font-family: 'Outfit';
      font-style: normal;
      font-weight: 100 900;
      font-display: swap;
      src: url('/fonts/outfit.woff2') format('woff2');
    }}
    :root {{
      --bg-base: #060913;
      --text-main: #f8fafc;
      --text-muted: #94a3b8;
      --glow-primary: #f43f5e;
      --glow-gradient: linear-gradient(135deg, #f43f5e 0%, #fb7185 100%);
      --glow-shadow: 0 0 35px -5px rgba(244, 63, 94, 0.4);
      --glass-bg: rgba(15, 23, 42, 0.75);
      --glass-border: rgba(244, 63, 94, 0.3);
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      min-height: 100vh;
      font-family: 'Inter', system-ui, sans-serif;
      background: radial-gradient(circle at 50% 30%, #1c1017 0%, #060913 75%);
      color: var(--text-main);
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 1.5rem;
    }}
    .ambient-glow {{
      position: fixed;
      width: 500px;
      height: 500px;
      border-radius: 50%;
      filter: blur(120px);
      opacity: 0.25;
      background: var(--glow-primary);
      pointer-events: none;
    }}
    .glass-card {{
      position: relative;
      z-index: 1;
      width: 100%;
      max-width: 460px;
      background: var(--glass-bg);
      backdrop-filter: blur(24px);
      border: 1px solid var(--glass-border);
      border-radius: 24px;
      padding: 2.5rem 2rem;
      text-align: center;
      box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6), var(--glow-shadow);
    }}
    .icon {{
      width: 64px;
      height: 64px;
      margin: 0 auto 1.5rem;
      border-radius: 20px;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 2rem;
      background: rgba(244, 63, 94, 0.15);
      border: 1px solid rgba(244, 63, 94, 0.4);
      color: #f87171;
    }}
    .title {{ font-family: 'Outfit', sans-serif; font-size: 1.4rem; font-weight: 700; margin-bottom: 0.5rem; }}
    .subtitle {{ font-size: 0.9rem; color: #fca5a5; line-height: 1.5; margin-bottom: 1.5rem; }}
    .btn {{
      display: inline-block;
      padding: 0.65rem 1.5rem;
      border-radius: 12px;
      font-size: 0.9rem;
      font-weight: 600;
      color: #ffffff;
      background: rgba(255, 255, 255, 0.1);
      border: 1px solid rgba(255, 255, 255, 0.2);
      text-decoration: none;
      transition: background 150ms ease;
    }}
    .btn:hover {{ background: rgba(255, 255, 255, 0.2); }}
  </style>
</head>
<body>
  <div class="ambient-glow"></div>
  <div class="glass-card">
    <div class="icon">⚠️</div>
    <h2 class="title">Authentication Error</h2>
    <p class="subtitle">{error_message}</p>
    <a href="/" class="btn">Return to Home</a>
  </div>
</body>
</html>"#)
}

#[allow(clippy::too_many_lines)]
async fn handle_callback(
    code: &str,
    state: &str,
    is_browser_get: bool,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let pending = match SESSIONS.take_pkce(state).await {
        Some(p) => p,
        None => {
            if is_browser_get {
                let html = render_callback_error_html("Invalid or expired state parameter. Please try logging in again.");
                return Ok(html_response(StatusCode::BAD_REQUEST, html));
            }
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid or expired state parameter" })),
            ));
        }
    };

    let issuer = CONFIG.oidc_issuer_url.as_deref().unwrap_or_default();
    let discovery_url = CONFIG.oidc_discovery_url.as_deref();
    let client_id = CONFIG.oidc_client_id.as_deref().unwrap_or_default();

    let discovery = match OIDC.get_discovery(issuer, discovery_url).await {
        Ok(d) => d,
        Err(e) => {
            if is_browser_get {
                let html = render_callback_error_html(&format!("OIDC discovery failed: {e}"));
                return Ok(html_response(StatusCode::INTERNAL_SERVER_ERROR, html));
            }
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("OIDC discovery failed: {e}") })),
            ));
        }
    };

    let token_res = match OIDC
        .exchange_code(
            &discovery.token_endpoint,
            client_id,
            CONFIG.oidc_client_secret.as_ref(),
            code,
            &pending.code_verifier,
            &pending.redirect_uri,
        )
        .await
    {
        Ok(t) => t,
        Err(e) => {
            error!("Token exchange failed: {e}");
            if is_browser_get {
                let html = render_callback_error_html(&format!("Token exchange failed: {e}"));
                return Ok(html_response(StatusCode::BAD_REQUEST, html));
            }
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Token exchange failed: {e}") })),
            ));
        }
    };

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
    let preferred_username = claims.preferred_username.clone();
    let email = claims.email.clone();
    let name = claims.name.clone();
    let sub = claims.sub.clone();
    let roles = claims.roles.unwrap_or_default();

    let username = preferred_username
        .as_deref()
        .or(email.as_deref())
        .or(sub.as_deref())
        .unwrap_or("oidc_user")
        .to_string();

    // Check admin status against all potential identifier claims
    let config_admin = CONFIG.is_user_admin_by_config(&username)
        || preferred_username.as_deref().is_some_and(|u| CONFIG.is_user_admin_by_config(u))
        || email.as_deref().is_some_and(|e| CONFIG.is_user_admin_by_config(e))
        || sub.as_deref().is_some_and(|s| CONFIG.is_user_admin_by_config(s))
        || name.as_deref().is_some_and(|n| CONFIG.is_user_admin_by_config(n));

    // Check admin status against roles (case-insensitively)
    let role_admin = roles.iter().any(|r| {
        let r_clean = r.trim().to_lowercase();
        r_clean == "admin"
            || r_clean == "administrator"
            || CONFIG
                .oidc_admin_roles
                .iter()
                .any(|ar| ar.trim().eq_ignore_ascii_case(r.trim()))
    });

    let existing_db_user = get_user_by_username(&username).await.ok().flatten();

    // 1. Check auto-registration if user does not exist in DB
    if existing_db_user.is_none() && !CONFIG.oidc_auto_register {
        warn!("User '{username}' attempted OIDC login but is not registered and auto-registration is disabled");
        if is_browser_get {
            let html = render_callback_error_html(&format!(
                "User registration is disabled and account '{username}' does not exist. Please contact an administrator."
            ));
            return Ok(html_response(StatusCode::FORBIDDEN, html));
        }
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": format!("User '{username}' is not registered and auto-registration is disabled") })),
        ));
    }

    // 2. Check user expiration if user exists in DB
    if let Some(ref db_user) = existing_db_user
        && db_user.is_expired()
    {
        warn!("User '{username}' attempted OIDC login but account has expired");
        if is_browser_get {
            let html = render_callback_error_html(&format!(
                "User account '{username}' has expired. Please contact an administrator."
            ));
            return Ok(html_response(StatusCode::FORBIDDEN, html));
        }
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": format!("User account '{username}' has expired") })),
        ));
    }

    let db_admin = existing_db_user.as_ref().is_some_and(|u| u.is_admin);

    let is_admin = db_admin || config_admin || role_admin;

    info!("User '{username}' successfully authenticated via OIDC (is_admin: {is_admin})");

    // Create or update user in database with full admin flag & preserved expires_at
    if let Err(e) = create_user(
        &username,
        existing_db_user.as_ref().and_then(|u| u.limit_to_models.as_ref()),
        existing_db_user.as_ref().and_then(|u| u.log_request_body),
        is_admin,
        existing_db_user.as_ref().and_then(|u| u.expires_at),
    )
    .await
    {
        error!("Failed to auto-create/update user in database: {e}");
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
        let html = render_callback_success_html(&username, is_admin, &session_token);
        let mut response = html_response(StatusCode::OK, html);

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
        let html = render_callback_error_html(&format!("OIDC authorization error: {err} ({desc})"));
        return Ok(html_response(StatusCode::BAD_REQUEST, html));
    }

    let code = match query.code {
        Some(c) => c,
        None => {
            let html = render_callback_error_html("Missing code parameter from identity provider.");
            return Ok(html_response(StatusCode::BAD_REQUEST, html));
        }
    };

    let state = match query.state {
        Some(s) => s,
        None => {
            let html = render_callback_error_html("Missing state parameter from identity provider.");
            return Ok(html_response(StatusCode::BAD_REQUEST, html));
        }
    };

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
        "expires_at": user.user.as_ref().and_then(|u| u.expires_at),
        "is_expired": user.user.as_ref().is_some_and(crate::db::user::User::is_expired),
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
