use redis::aio::ConnectionManager;
use sea_orm::DbConn;
use std::sync::{LazyLock, OnceLock};

use crate::project::configs::Configs;

pub static CONFIG: OnceLock<Configs> = OnceLock::new();

pub fn config() -> &'static Configs {
    CONFIG
        .get()
        .expect("application configuration must be initialized before serving requests")
}

pub static AUTH_JWT_CONFIG: LazyLock<auth_core::JwtConfig> =
    LazyLock::new(|| auth_core::JwtConfig {
        secret: config().jwt.secret.expose_secret().to_string(),
        expires_in_hours: config().jwt.expires_in_hours,
        refresh_secret: config().jwt.refresh_secret.expose_secret().to_string(),
        refresh_expires_in_days: config().jwt.refresh_expires_in_days,
    });

/// 应用状态，通过 axum::extract::State 注入到 handler 中
#[derive(Clone)]
pub struct AppState {
    pub db: DbConn,
    pub redis: ConnectionManager,
}
