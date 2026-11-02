use crate::auth::extractor::extract_client_ip;
use crate::db::alias::resolve_alias;
use crate::db::model::{get_exact_model_config, get_model_config};
use crate::db::openai_api::get_all_openai_apis;
use crate::db::user::{
    User, UserToken, get_user_and_token_by_token_hash, get_user_by_token_hash,
    hash_token, normalize_token_secret,
};
use crate::handlers::{GenerationType, Handler, get_handler};
use crate::utils::ModelRef;
use axum::Json;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use log::{debug, error, warn};
use serde_json::{Value, json};

pub async fn completions(
    authorization: Authorization,
    body: Json<Value>,
) -> Response {
    generate(GenerationType::Completions, authorization, body).await
}

pub async fn embeddings(
    authorization: Authorization,
    body: Json<Value>,
) -> Response {
    generate(GenerationType::Embeddings, authorization, body).await
}

pub async fn models(authorization: Authorization) -> Result<Json<Value>, StatusCode> {
    let mut seen_models = std::collections::HashSet::new();
    let mut models = Vec::new();

    let db_models = crate::db::model::get_all_models()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let group_allowed_models = crate::db::group::get_allowed_models_for_groups(&authorization.groups)
        .await
        .unwrap_or_default();

    for model in db_models {
        if model.id == "*" || crate::utils::is_glob_pattern(&model.id) || model.hidden.unwrap_or(false) {
            continue;
        }
        if let Some(users) = &model.limit_to_users
            && !crate::utils::is_user_allowed(users, &authorization.username)
            && !crate::utils::is_any_group_allowed(users, &authorization.groups)
        {
            continue;
        }
        // User account & group model restrictions (additive union)
        if let Some(limit_to) = &authorization.user.limit_to_models
            && !crate::utils::is_model_allowed(limit_to, &model.id)
            && !crate::utils::is_model_allowed(&group_allowed_models, &model.id)
        {
            continue;
        }
        // Token model restrictions
        if let Some(token_limit_to) = authorization.token.as_ref().and_then(|t| t.limit_to_models.as_ref())
            && !crate::utils::is_model_allowed(token_limit_to, &model.id)
        {
            continue;
        }

        if seen_models.insert(model.id.clone()) {
            models.push(json!({"id": model.id, "name": model.name.as_ref().unwrap_or(&model.id)}));
        }
    }

    let openai_apis = get_all_openai_apis()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for api_conf in openai_apis {
        if let Some(api_models) = &api_conf.models {
            for m in api_models {
                let full_id = if let Some(prefix) = &api_conf.prefix {
                    let p = prefix.trim_end_matches('/');
                    if m.starts_with(&format!("{p}/")) {
                        m.clone()
                    } else if m == p {
                        m.clone()
                    } else {
                        format!("{p}/{m}")
                    }
                } else {
                    m.clone()
                };

                if let Some(limit_to) = &authorization.user.limit_to_models
                    && !crate::utils::is_model_allowed(limit_to, &full_id)
                    && !crate::utils::is_model_allowed(limit_to, m)
                    && !crate::utils::is_model_allowed(&group_allowed_models, &full_id)
                    && !crate::utils::is_model_allowed(&group_allowed_models, m)
                {
                    continue;
                }

                if let Some(token_limit_to) = authorization.token.as_ref().and_then(|t| t.limit_to_models.as_ref())
                    && !crate::utils::is_model_allowed(token_limit_to, &full_id)
                    && !crate::utils::is_model_allowed(token_limit_to, m)
                {
                    continue;
                }

                if seen_models.insert(full_id.clone()) {
                    models.push(json!({"id": full_id, "name": full_id}));
                }
            }
        }
    }

    Ok(Json(json!({
        "data": models,
    })))
}

#[derive(Debug, Clone)]
pub enum TokenAuthResult {
    Valid {
        user: User,
        token: Option<UserToken>,
        groups: Vec<String>,
    },
    UserExpired(String),
    TokenExpired(String),
    IpForbidden(String),
    Invalid,
}

pub async fn check_token_auth_with_ip(
    raw_token: &str,
    client_ip: Option<std::net::IpAddr>,
) -> TokenAuthResult {
    if let Some(session) = crate::auth::session::SESSIONS.get_session(raw_token).await {
        let is_admin = session.is_admin
            || crate::config::CONFIG.is_user_admin_by_config(&session.username)
            || session
                .email
                .as_deref()
                .is_some_and(|e| crate::config::CONFIG.is_user_admin_by_config(e));

        let mut groups: Vec<String> = session.groups.into_iter().collect();
        if let Ok(manual) = crate::db::group::get_user_manual_groups(&session.username).await {
            for mg in manual {
                if !groups.contains(&mg) {
                    groups.push(mg);
                }
            }
        }
        groups.sort();

        if let Ok(Some(mut user)) = crate::db::user::get_user_by_username(&session.username).await {
            if user.is_expired() {
                return TokenAuthResult::UserExpired(user.username);
            }
            user.is_admin = user.is_admin || is_admin;
            return TokenAuthResult::Valid {
                user,
                token: None,
                groups,
            };
        }
        return TokenAuthResult::Valid {
            user: User {
                username: session.username,
                limit_to_models: None,
                log_request_body: None,
                is_admin,
                expires_at: None,
            },
            token: None,
            groups,
        };
    }

    // 1. Check normalized bearer token hash in DB
    let token_hash = hash_token(raw_token);
    if let Ok(Some((mut user, user_token))) = get_user_and_token_by_token_hash(&token_hash).await {
        if user.is_expired() {
            return TokenAuthResult::UserExpired(user.username);
        }
        if user_token.is_expired() {
            return TokenAuthResult::TokenExpired(
                user_token.name.clone().unwrap_or_else(|| user_token.preview_display()),
            );
        }
        if !user_token.is_ip_allowed(client_ip) {
            return TokenAuthResult::IpForbidden(format!(
                "Client IP {:?} is not allowed for this token",
                client_ip
            ));
        }
        let groups = crate::db::group::get_user_manual_groups(&user.username)
            .await
            .unwrap_or_default();
        user.is_admin = user.is_admin || crate::config::CONFIG.is_user_admin_by_config(&user.username);
        return TokenAuthResult::Valid {
            user,
            token: Some(user_token),
            groups,
        };
    }

    // 2. Check unhashed / legacy token in DB
    let normalized = normalize_token_secret(raw_token);
    if let Ok(Some((mut user, user_token))) = get_user_and_token_by_token_hash(normalized).await {
        if user.is_expired() {
            return TokenAuthResult::UserExpired(user.username);
        }
        if user_token.is_expired() {
            return TokenAuthResult::TokenExpired(
                user_token.name.clone().unwrap_or_else(|| user_token.preview_display()),
            );
        }
        if !user_token.is_ip_allowed(client_ip) {
            return TokenAuthResult::IpForbidden(format!(
                "Client IP {:?} is not allowed for this token",
                client_ip
            ));
        }
        let groups = crate::db::group::get_user_manual_groups(&user.username)
            .await
            .unwrap_or_default();
        user.is_admin = user.is_admin || crate::config::CONFIG.is_user_admin_by_config(&user.username);
        return TokenAuthResult::Valid {
            user,
            token: Some(user_token),
            groups,
        };
    }

    if let Ok(Some(mut user)) = get_user_by_token_hash(raw_token).await {
        if user.is_expired() {
            return TokenAuthResult::UserExpired(user.username);
        }
        let groups = crate::db::group::get_user_manual_groups(&user.username)
            .await
            .unwrap_or_default();
        user.is_admin = user.is_admin || crate::config::CONFIG.is_user_admin_by_config(&user.username);
        return TokenAuthResult::Valid {
            user,
            token: None,
            groups,
        };
    }

    TokenAuthResult::Invalid
}

pub async fn check_token_auth(token: &str) -> TokenAuthResult {
    check_token_auth_with_ip(token, None).await
}

pub async fn check_token(token: &str) -> Option<User> {
    match check_token_auth(token).await {
        TokenAuthResult::Valid { user, .. } => Some(user),
        _ => None,
    }
}

pub struct Authorization {
    pub username: String,
    pub user: User,
    pub token: Option<UserToken>,
    pub groups: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum AuthRejection {
    Unauthorized(String),
    Forbidden(String),
}

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        let (status, msg, code, err_type) = match self {
            AuthRejection::Unauthorized(m) => (
                StatusCode::UNAUTHORIZED,
                m,
                "invalid_api_key",
                "invalid_request_error",
            ),
            AuthRejection::Forbidden(m) => (
                StatusCode::FORBIDDEN,
                m,
                "user_expired",
                "access_denied",
            ),
        };

        let body = json!({
            "error": {
                "message": msg,
                "type": err_type,
                "code": code,
            },
            "message": msg
        });

        (status, [(header::CONTENT_TYPE, "application/json")], Json(body)).into_response()
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Authorization {
    type Rejection = AuthRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| AuthRejection::Unauthorized("Missing or invalid authorization header".to_string()))?;

        let client_ip = extract_client_ip(parts);

        match check_token_auth_with_ip(token, client_ip).await {
            TokenAuthResult::Valid { user, token: tok, groups } => {
                let username = user.username.clone();
                Ok(Authorization {
                    username,
                    user,
                    token: tok,
                    groups,
                })
            }
            TokenAuthResult::UserExpired(username) => {
                Err(AuthRejection::Forbidden(format!(
                    "User account '{username}' has expired"
                )))
            }
            TokenAuthResult::TokenExpired(ident) => {
                Err(AuthRejection::Forbidden(format!(
                    "API token '{ident}' has expired"
                )))
            }
            TokenAuthResult::IpForbidden(msg) => {
                Err(AuthRejection::Forbidden(msg))
            }
            TokenAuthResult::Invalid => {
                Err(AuthRejection::Unauthorized("Invalid authorization token".to_string()))
            }
        }
    }
}

async fn extract_models(
    authorization: &Authorization,
    body: &Json<Value>,
) -> Result<Vec<ModelRef>, StatusCode> {
    let Some(raw_models_str) = body.get("model").and_then(Value::as_str) else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let mut model_refs = Vec::new();
    for part in raw_models_str.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let resolved = resolve_alias(trimmed)
            .await
            .unwrap_or(None)
            .unwrap_or_else(|| trimmed.to_string());

        let model_ref: ModelRef = resolved.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
        model_refs.push(model_ref);
    }

    if model_refs.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    debug!("Received request for models {model_refs:?}");

    let openai_apis = get_all_openai_apis().await.unwrap_or_default();

    for model in &model_refs {
        let model_str = model.to_string();
        if model_str == "*" || crate::utils::is_glob_pattern(&model_str) {
            warn!("Direct execution of wildcard/glob model '{model_str}' is forbidden");
            return Err(StatusCode::BAD_REQUEST);
        }

        let matches_openai = openai_apis.iter().any(|api| api.matches_model(&model_str));

        let exact_model_conf = get_exact_model_config(&model_str).await.unwrap_or(None);
        let model_conf = if exact_model_conf.is_some() {
            exact_model_conf
        } else if matches_openai {
            None
        } else {
            get_model_config(&model_str).await.unwrap_or(None)
        };

        let group_allowed_models = crate::db::group::get_allowed_models_for_groups(&authorization.groups)
            .await
            .unwrap_or_default();

        // 1. Check user model limits (additive union with group permissions)
        let user_allowed = authorization
            .user
            .limit_to_models
            .as_ref()
            .is_none_or(|limited| {
                crate::utils::is_model_allowed(limited, &model_str)
                    || crate::utils::is_model_allowed(&group_allowed_models, &model_str)
            });

        // 2. Check token model limits
        let token_allowed = authorization
            .token
            .as_ref()
            .and_then(|t| t.limit_to_models.as_ref())
            .is_none_or(|limited| crate::utils::is_model_allowed(limited, &model_str));

        if !user_allowed || !token_allowed {
            warn!(
                "{} is not allowed to use {model} (user_allowed={user_allowed}, token_allowed={token_allowed})",
                authorization.username
            );
            return Err(StatusCode::FORBIDDEN);
        }

        if let Some(ref conf) = model_conf {
            if !conf
                .limit_to_users
                .as_ref()
                .is_none_or(|limited| {
                    crate::utils::is_user_allowed(limited, &authorization.username)
                        || crate::utils::is_any_group_allowed(limited, &authorization.groups)
                })
            {
                warn!("{} is not in allowed users or groups for {model}", authorization.username);
                return Err(StatusCode::FORBIDDEN);
            }
        } else if !matches_openai {
            return Err(StatusCode::NOT_FOUND);
        }
    }
    Ok(model_refs)
}

fn replace_model(body: &serde_json::Value, model: &ModelRef) -> serde_json::Value {
    let mut body = body.clone();
    body["model"] = json!(model.get_hf_id());
    body
}

async fn generate(
    generation_type: GenerationType,
    authorization: Authorization,
    body: Json<Value>,
) -> Response {
    let models = match extract_models(&authorization, &body).await {
        Ok(m) => m,
        Err(status) => return status.into_response(),
    };

    let prompt_tokens = crate::rate_limit::estimate_prompt_tokens(&body.0);

    // 1. Evaluate User-level rate limits across models
    let mut rate_headers = Vec::new();
    for model in &models {
        let model_str = model.to_string();
        match crate::rate_limit::check_rate_limits_for_user_and_groups(
            &authorization.username,
            &authorization.groups,
            &model_str,
            prompt_tokens,
        )
        .await
        {
            Ok(headers) => {
                rate_headers.extend(headers);
            }
            Err(rejection) => {
                return rejection.into_response();
            }
        }

        // 2. Evaluate Token-level rate limits across models
        if let Some(ref tok) = authorization.token {
            let token_ident = tok.name.as_deref().unwrap_or(tok.token_prefix.as_deref().unwrap_or("token"));
            match crate::rate_limit::check_token_rate_limits(
                &tok.token_hash,
                token_ident,
                &model_str,
                prompt_tokens,
            )
            .await
            {
                Ok(headers) => {
                    rate_headers.extend(headers);
                }
                Err(rejection) => {
                    return rejection.into_response();
                }
            }
        }
    }

    let token_hash_ref = authorization.token.as_ref().map(|t| t.token_hash.as_str());

    for model in models {
        let body_val = replace_model(&body.0, &model);
        match get_handler()
            .generate(
                generation_type,
                &model,
                &body_val,
                &authorization.username,
                authorization.user.log_request_body,
                prompt_tokens,
                token_hash_ref,
            )
            .await
        {
            Ok(mut res) => {
                if !rate_headers.is_empty() {
                    res.rate_limit_headers = Some(rate_headers);
                }
                return res.into_response();
            }
            Err(err) => {
                error!("Error while generating using model: {err:?}");
            }
        }
    }

    (
        StatusCode::BAD_GATEWAY,
        Json(json!({"error": "Error while generating using model"})),
    )
        .into_response()
}
