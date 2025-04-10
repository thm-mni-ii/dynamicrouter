use crate::config::{User, CONFIG};
use crate::factories::models::{DynModel, ModelResponse, RetryingModel};
use crate::factories::{get_factory, ModelFactory};
use crate::utils::ModelRef;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::Json;
use log::{debug, error, warn};
use serde_json::{json, Value};
use sha2::Digest;

pub async fn completions(
    authorization: Authorization<'_>,
    body: Json<Value>,
) -> Result<ModelResponse, StatusCode> {
    let model = get_model(&authorization, &body).await?;
    match model.generate(&body.0).await {
        Ok(res) => Ok(res),
        Err(err) => {
            error!("Error while generating using model: {err:?}");
            Err(StatusCode::BAD_GATEWAY)
        }
    }
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
        .map(|(key, model)| json!({"id": key, "name": model.name.as_ref().unwrap_or(key)}))
        .collect::<Vec<Value>>();

    Ok(Json(json!({
        "data": models,
    })))
}

pub async fn generate(
    authorization: Authorization<'_>,
    mut body: Json<Value>,
) -> Result<ModelResponse, StatusCode> {
    let model = get_model(&authorization, &body).await?;
    body.0["model"].take();

    match model.invoke("generate", &body.0).await {
        Some(Ok(res)) => Ok(res),
        Some(Err(err)) => {
            error!("Error while generating using model: {err:?}");
            Err(StatusCode::BAD_GATEWAY)
        }
        None => {
            warn!("Backend does not support invoke");
            Err(StatusCode::BAD_GATEWAY)
        }
    }
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
            .and_then(|token| {
                let mut hasher = sha2::Sha256::new();
                hasher.update(token.as_bytes());
                let hash = hasher.finalize();
                let encoded = hex::encode(hash);
                CONFIG
                    .users
                    .iter()
                    .find(|(_name, u)| u.token_hashes.contains(&encoded))
            })
            .map(|(username, user)| Authorization { username, user })
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

async fn get_model(
    authorization: &Authorization<'_>,
    body: &Json<Value>,
) -> Result<DynModel, StatusCode> {
    let Some(model) = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(|m| {
            CONFIG
                .aliases
                .get(m)
                .map_or_else(|| m.to_string(), |a| a.to.clone())
        })
        .and_then(|m| m.parse::<ModelRef>().ok())
    else {
        return Err(StatusCode::BAD_REQUEST);
    };
    debug!("Received request for model {model:?}");

    if let Some(model_conf) = CONFIG
        .models
        .get(&model.to_string())
        .or_else(|| CONFIG.models.get("*"))
    {
        if let Some(limited) = &model_conf.limit_to_users {
            if !limited.contains(authorization.username) {
                warn!("{} is not allowed to use {model}", authorization.username);
                return Err(StatusCode::FORBIDDEN);
            }
        }
    } else {
        return Err(StatusCode::NOT_FOUND);
    }

    let model: DynModel = match get_factory().create(model).await {
        Ok(model) => RetryingModel::new_dyn(model),
        Err(err) => {
            error!("Error while creating model: {err:?}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    Ok(model)
}
