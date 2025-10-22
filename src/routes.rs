use crate::config::{CONFIG, User};
use crate::handlers::{Handler, get_handler};
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
    let models = generate(&authorization, &body)?;

    let mut result = Err(StatusCode::BAD_REQUEST);
    for model in models {
        match get_handler().generate(&model, &body.0).await {
            Ok(res) => {
                result = Ok(res);
            }
            Err(err) => {
                error!("Error while generating using model: {err:?}");
                result = Err(StatusCode::BAD_GATEWAY);
            }
        }
    }

    result
}

pub async fn models(authorization: Authorization<'_>) -> Result<Json<Value>, StatusCode> {
    let models = CONFIG
        .models
        .iter()
        .filter(|(name, model)| !model.hidden.unwrap_or(*name == "*"))
        .filter(|(_, model)| match &model.limit_to_users {
            Some(users) => users.contains(authorization.username),
            None => true,
        })
        .filter(|(name, _model)| {
            authorization
                .user
                .limit_to_models
                .as_ref()
                .is_none_or(|limit_to| limit_to.contains(*name))
        })
        .map(|(key, model)| json!({"id": key, "name": model.name.as_ref().unwrap_or(key)}))
        .collect::<Vec<Value>>();

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

fn generate(
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
        if let Some(model_conf) = CONFIG
            .models
            .get(&model.to_string())
            .or_else(|| CONFIG.models.get("*"))
        {
            if !(model_conf
                .limit_to_users
                .as_ref()
                .is_none_or(|limited| limited.contains(authorization.username))
                && authorization
                    .user
                    .limit_to_models
                    .as_ref()
                    .is_none_or(|limited| limited.contains(&model.to_string())))
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
