use crate::db::alias::resolve_alias;
use crate::db::model::get_model_config;
use crate::db::openai_api::get_all_openai_apis;
use crate::db::user::{User, get_user_by_token_hash};
use crate::handlers::{GenerationType, Handler, get_handler};
use crate::utils::ModelRef;
use crate::utils::response::ModelResponse;
use axum::Json;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use log::{debug, error, warn};
use serde_json::{Value, json};
use sha2::Digest;

pub async fn completions(
    authorization: Authorization,
    body: Json<Value>,
) -> Result<ModelResponse, StatusCode> {
    generate(GenerationType::Completions, authorization, body).await?
}

pub async fn embeddings(
    authorization: Authorization,
    body: Json<Value>,
) -> Result<ModelResponse, StatusCode> {
    generate(GenerationType::Embeddings, authorization, body).await?
}

pub async fn models(authorization: Authorization) -> Result<Json<Value>, StatusCode> {
    let mut seen_models = std::collections::HashSet::new();
    let mut models = Vec::new();

    let db_models = crate::db::model::get_all_models()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for model in db_models {
        if model.hidden.unwrap_or(model.id == "*") {
            continue;
        }
        if let Some(users) = &model.limit_to_users
            && !users.contains(&authorization.username)
        {
            continue;
        }
        if let Some(limit_to) = &authorization.user.limit_to_models
            && !limit_to.contains(&model.id)
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
                    format!("{p}/{m}")
                } else {
                    m.clone()
                };

                if let Some(limit_to) = &authorization.user.limit_to_models
                    && !limit_to.contains(&full_id)
                    && !limit_to.contains(m)
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

pub async fn check_token(token: &str) -> Option<User> {
    let mut hasher = sha2::Sha256::new();
    hasher.update(token.as_bytes());
    let hash = hasher.finalize();
    let encoded = hex::encode(hash);

    get_user_by_token_hash(&encoded).await.ok().flatten()
}

pub struct Authorization {
    pub username: String,
    pub user: User,
}

impl<S: Send + Sync> FromRequestParts<S> for Authorization {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let user = check_token(token).await.ok_or(StatusCode::UNAUTHORIZED)?;
        let username = user.username.clone();

        Ok(Authorization { username, user })
    }
}

async fn extract_models(
    authorization: &Authorization,
    body: &Json<Value>,
) -> Result<Vec<ModelRef>, StatusCode> {
    let Some(raw_models_str) = body.get("model").and_then(|v| v.as_str()) else {
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
        let matches_openai = openai_apis.iter().any(|api| api.matches_model(&model_str));

        let model_conf = get_model_config(&model_str).await.unwrap_or(None);

        if let Some(ref conf) = model_conf {
            if !(conf
                .limit_to_users
                .as_ref()
                .is_none_or(|limited| limited.contains(&authorization.username))
                && authorization
                    .user
                    .limit_to_models
                    .as_ref()
                    .is_none_or(|limited| limited.contains(&model_str)))
            {
                warn!("{} is not allowed to use {model}", authorization.username);
                return Err(StatusCode::FORBIDDEN);
            }
        } else if matches_openai {
            if !authorization
                .user
                .limit_to_models
                .as_ref()
                .is_none_or(|limited| limited.contains(&model_str))
            {
                warn!("{} is not allowed to use {model}", authorization.username);
                return Err(StatusCode::FORBIDDEN);
            }
        } else {
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
) -> Result<Result<ModelResponse, StatusCode>, StatusCode> {
    let models = extract_models(&authorization, &body).await?;

    let mut result = Err(StatusCode::BAD_REQUEST);
    for model in models {
        let body = replace_model(&body.0, &model);
        match get_handler()
            .generate(
                generation_type,
                &model,
                &body,
                &authorization.username,
                authorization.user.log_request_body,
            )
            .await
        {
            Ok(res) => {
                result = Ok(res);
            }
            Err(err) => {
                error!("Error while generating using model: {err:?}");
                result = Err(StatusCode::BAD_GATEWAY);
            }
        }
    }

    Ok(result)
}
