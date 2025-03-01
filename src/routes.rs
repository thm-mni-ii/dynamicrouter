use crate::config::CONFIG;
use crate::handler::factories::{get_factory, ModelFactory};
use crate::handler::models::{DynModel, ModelResponse, RetryingModel};
use axum::http::{header, HeaderMap, StatusCode};
use axum::Json;
use log::{debug, error};
use serde_json::{json, Value};
use sha2::Digest;
use crate::handler::utils::ModelRef;

pub async fn completions(headers: HeaderMap, body: Json<Value>) -> Result<ModelResponse, StatusCode> {
    if !verify_authorization(&headers) {
        return Err(StatusCode::UNAUTHORIZED)
    }

    let Some(model) = body.get("model").and_then(|v| v.as_str()).and_then(|m| m.parse::<ModelRef>().ok()) else { return Err(StatusCode::BAD_REQUEST) };
    debug!("Received request for model {model:?}");

    let model: DynModel = match get_factory().create(model).await {
        Ok(model) => RetryingModel::new_dyn(model),
        Err(err) => {
            error!("Error while creating model: {err:?}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    };
    match model.generate(&body.0).await {
        Ok(res) => Ok(res),
        Err(err) => {
            error!("Error while generating using model: {err:?}");
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

pub async fn models(headers: HeaderMap) -> Result<Json<Value>, StatusCode> {
    if !verify_authorization(&headers) {
        return Err(StatusCode::UNAUTHORIZED)
    }

    Ok(Json(json!({
        "data": CONFIG.advertise_models.iter().map(|model| json!({"id": model, "name": model})).collect::<Vec<Value>>()
    })))
}

fn verify_authorization(headers: &HeaderMap) -> bool {
    headers.get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .is_some_and(|token| {
            let mut hasher = sha2::Sha256::new();
            hasher.update(token.as_bytes());
            let hash = hasher.finalize();
            let encoded = hex::encode(hash);
            CONFIG.hashes.contains(&encoded)
        })
}
