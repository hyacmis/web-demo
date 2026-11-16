use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Redirect;
use serde::Deserialize;
use tracing::info;

use super::schemas::{LoginInput, LoginOutput, RefreshInput, RegisterInput, SendSmsCodeInput};
use crate::constants::{AUTH_JWT_CONFIG, AppState, config};
use crate::helper::sms;
use crate::project::error::{ApiResult, AppError, ok};
use crate::project::extractor::ValidatedJson;

/// 发送验证码
#[utoipa::path(
    post,
    path = "/auth/sms-code",
    tag = "认证",
    request_body = SendSmsCodeInput,
    security(()),
    responses(
        (status = 200),
        (status = 400, description = "Validation error")
    )
)]
pub async fn send_sms_code(
    State(state): State<AppState>,
    ValidatedJson(input): ValidatedJson<SendSmsCodeInput>,
) -> ApiResult<()> {
    sms::send_code(&state.redis, &input.phone).await?;
    ok(())
}

/// 用户注册（带验证码验证）
#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "认证",
    request_body = RegisterInput,
    security(()),
    responses(
        (status = 200),
        (status = 400, description = "Validation error")
    )
)]
pub async fn register(
    State(state): State<AppState>,
    ValidatedJson(input): ValidatedJson<RegisterInput>,
) -> ApiResult<()> {
    auth_core::register(
        &state.db,
        &input.phone,
        &input.password,
        input.nickname,
        input.name,
        || async { sms::verify_code(&state.redis, &input.phone, &input.code).await },
    )
    .await?;
    ok(())
}

/// 用户登录
#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "认证",
    request_body = LoginInput,
    security(()),
    responses(
        (status = 200, body = LoginOutput),
        (status = 400, description = "Validation error")
    )
)]
pub async fn login(
    State(state): State<AppState>,
    ValidatedJson(input): ValidatedJson<LoginInput>,
) -> ApiResult<LoginOutput> {
    auth_core::login(&state.db, &input.phone, &input.password, &AUTH_JWT_CONFIG)
        .await
        .map(Into::into)
        .map_err(AppError::from)
        .and_then(ok)
}

/// 刷新 Token
#[utoipa::path(
    post,
    path = "/auth/refresh",
    tag = "认证",
    request_body = RefreshInput,
    security(()),
    responses(
        (status = 200, body = LoginOutput),
        (status = 401, description = "Refresh Token 无效")
    )
)]
pub async fn refresh(
    State(state): State<AppState>,
    ValidatedJson(input): ValidatedJson<RefreshInput>,
) -> ApiResult<LoginOutput> {
    auth_core::refresh(
        &state.db,
        &state.redis,
        &input.refresh_token,
        &AUTH_JWT_CONFIG,
    )
    .await
    .map(Into::into)
    .map_err(AppError::from)
    .and_then(ok)
}

/// OAuth2 回调参数
#[derive(Debug, Deserialize)]
pub struct OauthCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// OAuth2 登录 - 重定向到对应提供商授权页
#[utoipa::path(
    get,
    path = "/auth/oauth2/{provider}",
    tag = "认证",
    security(()),
    responses(
        (status = 302, description = "重定向到 OAuth2 授权页"),
        (status = 500, description = "内部错误")
    )
)]
pub async fn oauth2_login(
    State(state): State<AppState>,
    Path(provider): Path<String>,
) -> Result<Redirect, AppError> {
    let authorization = auth_core::oauth_authorization(&provider, &config().oauth2.providers)?;

    auth_core::store_oauth_state(&state.redis, &provider, &authorization.state).await?;

    Ok(Redirect::temporary(&authorization.url))
}

/// OAuth2 回调 - 处理授权码，创建/关联用户，返回 JWT token
#[utoipa::path(
    get,
    path = "/auth/oauth2/{provider}/callback",
    tag = "认证",
    params(
        ("code" = String, Query, description = "授权码"),
        ("state" = Option<String>, Query, description = "CSRF token")
    ),
    security(()),
    responses(
        (status = 200, body = LoginOutput),
        (status = 400, description = "OAuth2 错误"),
        (status = 500, description = "内部错误")
    )
)]
pub async fn oauth2_callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(params): Query<OauthCallbackParams>,
) -> ApiResult<LoginOutput> {
    // 0. 检查提供商是否返回错误
    if let Some(error) = params.error {
        let desc = params.error_description.unwrap_or_default();
        tracing::warn!("OAuth2 授权被拒绝 ({}): {} - {}", provider, error, desc);
        return Err(AppError::Api(
            StatusCode::BAD_REQUEST,
            "OAuth2 授权被拒绝".to_string(),
        ));
    }

    let code = params
        .code
        .ok_or_else(|| AppError::Api(StatusCode::BAD_REQUEST, "缺少授权码".to_string()))?;

    // 1. 验证 CSRF token（原子操作：获取并删除）
    let csrf_state = params
        .state
        .ok_or_else(|| AppError::Api(StatusCode::BAD_REQUEST, "无效的请求参数".to_string()))?;

    auth_core::consume_oauth_state(&state.redis, &provider, &csrf_state).await?;

    // 2. 换取用户资料（auth-core 负责 provider 适配）
    let user_info =
        auth_core::exchange_oauth_code(&provider, &config().oauth2.providers, &code).await?;

    // 3. 查找或创建本地用户并绑定 OAuth 身份
    let user = auth_core::link_or_create_account(&state.db, &provider, &user_info).await?;

    // 4. 生成 JWT token
    let output = auth_core::generate_token_pair(user.id, &AUTH_JWT_CONFIG)?.into();

    info!(
        "OAuth2 登录成功 -> 提供商: {}, 用户: {}",
        provider, user.nickname
    );

    ok(output)
}
