#![warn(clippy::pedantic)]
#![allow(clippy::match_wild_err_arm)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]

mod routes;
mod handler;
mod config;
mod runtime;
mod db;

use axum::Router;
use axum::routing::{get, post};
use log::{info, LevelFilter};
use crate::config::CONFIG;
use db::init_db;
use crate::handler::factories::init_factories;
use crate::runtime::RUNTIME;

fn main() {
    RUNTIME.block_on(async_main());
}

async fn async_main() {
    env_logger::builder().filter_level(LevelFilter::Info).parse_default_env().init();

    init_db().await;
    init_factories().await;

    // build our application with a single route
    let app = Router::new()
        .route("/v1/chat/completions", post(routes::completions))
        .route("/v1/models", get(routes::models));

    let port = CONFIG.port;

    info!("Listening on :{port}");

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
