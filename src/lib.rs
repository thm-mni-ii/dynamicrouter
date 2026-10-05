#![warn(clippy::pedantic)]
#![allow(clippy::match_wild_err_arm)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::implicit_hasher)]
#![allow(clippy::too_many_lines)]

pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod handlers;
pub mod routes;
pub mod runtime;
pub mod ui;
pub mod utils;
