pub mod aliases;
pub mod backends;
pub mod factory_gpus;
pub mod instances;
pub mod logs;
pub mod me;
pub mod metrics;
pub mod models;
pub mod openai_apis;
pub mod users;

use axum::Router;

pub fn api_routes() -> Router {
    Router::new()
        .nest("/me", me::me_routes())
        .nest("/models", models::models_routes())
        .nest("/openai-apis", openai_apis::openai_apis_routes())
        .nest("/users", users::users_routes())
        .nest("/aliases", aliases::aliases_routes())
        .nest("/backends", backends::backends_routes())
        .nest("/backend-model-configs", backends::backend_model_configs_global_routes())
        .nest("/factory-gpus", factory_gpus::factory_gpus_routes())
        .nest("/instances", instances::instances_routes())
        .nest("/logs", logs::logs_routes())
        .nest("/metrics", metrics::metrics_routes())
}
