use crate::config::CONFIG;
use crate::handler::factories::{get_factory, ModelFactory};
use crate::handler::models::{DynModel, ModelResponse, RetryingModel};
use axum::http::StatusCode;
use axum::Json;
use log::error;
use serde_json::{json, Value};

pub async fn completions(body: Json<Value>) -> Result<ModelResponse, StatusCode> {
    let Some(model) = body.get("model").and_then(|v| v.as_str()) else { return Err(StatusCode::BAD_REQUEST) };

    let model: DynModel = match get_factory().create(model.to_string()).await {
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

pub async fn models() -> Json<Value> {
    Json(json!({
        "data": CONFIG.advertise_models.iter().map(|model| json!({"id": model, "name": model})).collect::<Vec<Value>>()
    }))
}
