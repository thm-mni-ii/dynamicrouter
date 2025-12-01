#![warn(clippy::pedantic)]
#![allow(clippy::match_wild_err_arm)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::match_wildcard_for_single_variants)]

mod config;
mod db;
mod handlers;
mod routes;
mod runtime;
mod ui;
mod utils;

use crate::config::{CONFIG, TlsConfig};
use crate::handlers::init_handlers;
use crate::runtime::RUNTIME;
use crate::ui::new_static_router;
use axum::Router;
use axum::routing::{get, post};
use axum_server::tls_rustls::RustlsConfig;
use db::connection::init_db;
use log::{LevelFilter, debug, error, info};
use rustls::crypto::ring;
use std::process::exit;
use std::time::Duration;
use tokio::{signal, spawn};

fn main() {
    RUNTIME.block_on(async_main());
}

async fn async_main() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    debug!("Starting with config {:?}", *CONFIG);

    ring::default_provider().install_default().unwrap();
    init_db().await;
    init_handlers().await;

    let app = Router::new()
        .merge(new_static_router())
        .route("/v1/chat/completions", post(routes::completions))
        .route("/v1/models", get(routes::models));

    let port = CONFIG.port;

    let handle = axum_server::Handle::new();
    spawn(shutdown_signal(handle.clone()));

    let addr = match format!("0.0.0.0:{port}").parse() {
        Ok(addr) => addr,
        Err(err) => {
            error!("Failed to parse bind address: {err}");
            exit(1);
        }
    };

    if let Some(tls_config) = &CONFIG.tls {
        let config_result = match tls_config {
            TlsConfig::TlsPathConfig {
                cert_path,
                key_path,
            } => RustlsConfig::from_pem_chain_file(cert_path, key_path).await,
            TlsConfig::TlsInlineConfig { cert, key } => {
                RustlsConfig::from_pem(cert.as_bytes().to_vec(), key.as_bytes().to_vec()).await
            }
        };

        let conf = match config_result {
            Ok(conf) => conf,
            Err(err) => {
                error!("Failed to load tls config: {err}");
                exit(1);
            }
        };

        info!("Listening with HTTPS on :{port}");

        axum_server::bind_rustls(addr, conf)
            .handle(handle)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        info!("Listening with HTTP on :{port}");

        axum_server::bind(addr)
            .handle(handle)
            .serve(app.into_make_service())
            .await
            .unwrap();
    }
}

async fn shutdown_signal(handle: axum_server::Handle) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    handle.graceful_shutdown(Some(Duration::from_secs(10)));
}
