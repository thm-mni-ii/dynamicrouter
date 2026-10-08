use crate::auth::AdminUser;
use crate::db::model::{
    ModelConfig, ModelHandlerConfig, create_model, delete_model, get_all_models, get_exact_model_config,
};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use secrecy::ExposeSecret;
use serde_json::json;
use std::collections::HashSet;

pub fn models_routes() -> Router {
    Router::new()
        .route("/", get(list_models).post(create_new_model))
        .route("/{*id}", get(get_model).put(update_model_config).delete(remove_model))
}

async fn list_models(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut models = get_all_models().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch models: {e}") })),
        )
    })?;

    let existing_ids: HashSet<String> = models.iter().map(|m| m.id.clone()).collect();

    // Include Cloud API models configured via OpenAI APIs so they appear in the Admin Models table
    if let Ok(openai_apis) = crate::db::openai_api::get_all_openai_apis().await {
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

                    if !existing_ids.contains(&full_id) {
                        let display_name = if let Some(prefix) = &api_conf.prefix {
                            let p = prefix.trim_end_matches('/');
                            m.strip_prefix(&format!("{p}/")).unwrap_or(m).to_string()
                        } else {
                            m.clone()
                        };
                        models.push(ModelConfig {
                            id: full_id.clone(),
                            name: Some(display_name),
                            timeout: api_conf.timeout,
                            limit_to_users: None,
                            hidden: Some(false),
                            handler: ModelHandlerConfig::OpenAi(crate::db::model::OpenAiHandlerConfig {
                                api: Some(api_conf.id.clone()),
                                url: Some(api_conf.url.clone()),
                                token: None,
                                model: Some(m.clone()),
                                timeout: api_conf.timeout,
                                headers: None,
                            }),
                        });
                    }
                }
            }
        }
    }

    Ok(Json(json!({ "models": models })))
}

async fn get_model(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let model = get_exact_model_config(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch model: {e}") })),
        )
    })?;

    if let Some(m) = model {
        Ok(Json(json!(m)))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Model '{id}' not found") })),
        ))
    }
}

async fn create_new_model(
    _admin: AdminUser,
    Json(payload): Json<ModelConfig>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    create_model(&payload).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to create model: {e}") })),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "created",
            "id": payload.id,
        })),
    ))
}

async fn update_model_config(
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(mut payload): Json<ModelConfig>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    payload.id = id.clone();
    create_model(&payload).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to update model: {e}") })),
        )
    })?;

    Ok(Json(json!({
        "status": "updated",
        "id": id,
    })))
}

async fn remove_model(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_model(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete model: {e}") })),
        )
    })?;

    if deleted {
        return Ok(Json(json!({ "status": "deleted", "id": id })));
    }

    // If not in models table, check if it's an upstream model from openai_apis
    if let Ok(apis) = crate::db::openai_api::get_all_openai_apis().await {
        for api in apis {
            let mut models_set = api.models.clone().unwrap_or_default();
            let stripped = api.map_model_name(&id);
            if models_set.remove(&id) || models_set.remove(&stripped) {
                let models_opt = if models_set.is_empty() {
                    None
                } else {
                    Some(models_set)
                };
                let _ = crate::db::openai_api::create_openai_api(
                    &api.id,
                    api.url.as_str(),
                    api.token.as_ref().map(|t| t.expose_secret().as_str()),
                    api.prefix.as_deref(),
                    models_opt.as_ref(),
                    api.timeout,
                    api.headers.as_ref(),
                )
                .await;
                return Ok(Json(json!({ "status": "deleted", "id": id })));
            }
        }
    }

    Err((
        StatusCode::NOT_FOUND,
        Json(json!({ "error": format!("Model '{id}' not found") })),
    ))
}
