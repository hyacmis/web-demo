use axum::Router;
use axum::routing::{get, post};

use crate::constants::AppState;

pub mod schemas;
pub mod view;

/// 认证相关路由
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/sms-code", post(view::send_sms_code))
        .route("/register", post(view::register))
        .route("/login", post(view::login))
        .route("/refresh", post(view::refresh))
        .route("/{provider}", get(view::oauth2_login))
        .route("/{provider}/callback", get(view::oauth2_callback))
}
