use axum::{
    body::Body,
    extract::{FromRequestParts, State},
    http::{Request, StatusCode, request::Parts},
    middleware::Next,
    response::Response,
};
use entity::prelude::Account;

use crate::{
    constants::{AUTH_JWT_CONFIG, AppState},
    project::error::AppError,
};

/// 认证上下文，包含当前登录用户信息
#[derive(Clone)]
pub struct AuthContext {
    pub user_id: i32,
    pub user_type: i16,
}

impl<S: Send + Sync> FromRequestParts<S> for AuthContext {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .ok_or_else(|| AppError::Api(StatusCode::UNAUTHORIZED, "缺少认证信息".to_string()))
    }
}

/// 公开路径（不需要鉴权）
const PUBLIC_PATHS: &[&str] = &[
    "/",
    "/api/v1/auth/sms-code",
    "/api/v1/auth/register",
    "/api/v1/auth/login",
    "/api/v1/auth/refresh",
    "/docs",
    "/scalar",
    "/openapi.json",
    "/favicon.ico",
];

/// 检查路径是否公开（不需要鉴权）
fn is_public_path(path: &str) -> bool {
    let path = path.trim_end_matches('/');
    let path = if path.is_empty() { "/" } else { path };
    if PUBLIC_PATHS.contains(&path) {
        return true;
    }
    if ["/docs", "/scalar"].iter().any(|prefix| {
        path.strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
    }) {
        return true;
    }
    // 匹配 /api/v1/auth/{provider} 和 /api/v1/auth/{provider}/callback
    if let Some(rest) = path.strip_prefix("/api/v1/auth/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() == 1 || (parts.len() == 2 && parts[1] == "callback") {
            return true;
        }
    }
    false
}

/// JWT 认证中间件
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let path = request.uri().path();

    // 公开路径直接放行
    if is_public_path(path) {
        return Ok(next.run(request).await);
    }

    // 从请求头获取 Authorization
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::Api(StatusCode::UNAUTHORIZED, "缺少认证信息".to_string()))?;

    // 提取 Bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Api(StatusCode::UNAUTHORIZED, "认证格式错误".to_string()))?;

    // 验证 token
    let claims = auth_core::verify_access_token(token, &AUTH_JWT_CONFIG)
        .map_err(|_| AppError::Api(StatusCode::UNAUTHORIZED, "Token 无效或已过期".to_string()))?;

    let user = Account::find_by_id(claims.sub)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::Api(StatusCode::UNAUTHORIZED, "用户不存在或已停用".to_string()))?;

    // 将用户信息插入请求扩展
    request.extensions_mut().insert(AuthContext {
        user_id: claims.sub,
        user_type: user.r#type,
    });

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::is_public_path;

    #[test]
    fn keeps_health_and_nested_docs_public() {
        assert!(is_public_path("/"));
        assert!(is_public_path("/docs/openapi.json"));
        assert!(is_public_path("/scalar/"));
        assert!(!is_public_path("/docs-private"));
    }

    #[test]
    fn public_auth_paths_are_limited_to_login_callbacks() {
        assert!(is_public_path("/api/v1/auth/login"));
        assert!(is_public_path("/api/v1/auth/github"));
        assert!(is_public_path("/api/v1/auth/github/callback"));
        assert!(!is_public_path("/api/v1/auth/register/extra"));
        assert!(!is_public_path("/api/v1/user"));
    }
}
