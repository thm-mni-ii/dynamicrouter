use crate::config::{CONFIG, User};
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
    authorization: Authorization<'_>,
    body: Json<Value>,
) -> Result<ModelResponse, StatusCode> {
    generate(GenerationType::Completions, authorization, body).await?
}

pub async fn embeddings(
    authorization: Authorization<'_>,
    body: Json<Value>,
) -> Result<ModelResponse, StatusCode> {
    generate(GenerationType::Embeddings, authorization, body).await?
}

pub async fn models(authorization: Authorization<'_>) -> Result<Json<Value>, StatusCode> {
    let mut seen_models = std::collections::HashSet::new();
    let mut models = Vec::new();

    for (name, model) in &CONFIG.models {
        if model.hidden.unwrap_or(*name == "*") {
            continue;
        }
        if let Some(users) = &model.limit_to_users
            && !users.contains(authorization.username)
        {
            continue;
        }
        if let Some(limit_to) = &authorization.user.limit_to_models
            && !limit_to.contains(name)
        {
            continue;
        }
        if seen_models.insert(name.clone()) {
            models.push(json!({"id": name, "name": model.name.as_ref().unwrap_or(name)}));
        }
    }

    for api_conf in CONFIG.openai_apis.values() {
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

pub fn check_token<'r>(token: &str) -> Option<(&'r String, &'r User)> {
    let mut hasher = sha2::Sha256::new();
    hasher.update(token.as_bytes());
    let hash = hasher.finalize();
    let encoded = hex::encode(hash);
    CONFIG
        .users
        .iter()
        .find(|(_name, u)| u.token_hashes.contains(&encoded))
}

#[allow(dead_code)]
pub struct Authorization<'a> {
    username: &'a String,
    user: &'a User,
}

impl<S: Send + Sync> FromRequestParts<S> for Authorization<'_> {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|header| header.to_str().ok())
            .and_then(|header| header.strip_prefix("Bearer "))
            .and_then(check_token)
            .map(|(username, user)| Authorization { username, user })
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

fn extract_models(
    authorization: &Authorization,
    body: &Json<Value>,
) -> Result<Vec<ModelRef>, StatusCode> {
    let Some(models) = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(|m| {
            CONFIG
                .aliases
                .get(m)
                .map_or_else(|| m.to_string(), |a| a.to.clone())
        })
        .and_then(|ms| {
            ms.split(',')
                .map(|m| m.parse().ok())
                .collect::<Option<Vec<ModelRef>>>()
        })
    else {
        return Err(StatusCode::BAD_REQUEST);
    };
    debug!("Received request for models {models:?}");

    for model in &models {
        let model_str = model.to_string();
        let matches_openai = CONFIG
            .openai_apis
            .values()
            .any(|api| api.matches_model(&model_str));

        if let Some(model_conf) = CONFIG.models.get(&model_str) {
            if !(model_conf
                .limit_to_users
                .as_ref()
                .is_none_or(|limited| limited.contains(authorization.username))
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
        } else if let Some(wildcard_conf) = CONFIG.models.get("*") {
            if !(wildcard_conf
                .limit_to_users
                .as_ref()
                .is_none_or(|limited| limited.contains(authorization.username))
                && authorization
                    .user
                    .limit_to_models
                    .as_ref()
                    .is_none_or(|limited| limited.contains(&model_str)))
            {
                warn!("{} is not allowed to use {model}", authorization.username);
                return Err(StatusCode::FORBIDDEN);
            }
        } else {
            return Err(StatusCode::NOT_FOUND);
        }
    }
    Ok(models)
}

fn replace_model(body: &serde_json::Value, model: &ModelRef) -> serde_json::Value {
    let mut body = body.clone();
    body["model"] = json!(model.get_hf_id());
    body
}

async fn generate(
    generation_type: GenerationType,
    authorization: Authorization<'_>,
    body: Json<Value>,
) -> Result<Result<ModelResponse, StatusCode>, StatusCode> {
    let models = extract_models(&authorization, &body)?;

    let mut result = Err(StatusCode::BAD_REQUEST);
    for model in models {
        let body = replace_model(&body.0, &model);
        match get_handler()
            .generate(generation_type, &model, &body, authorization.username)
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
