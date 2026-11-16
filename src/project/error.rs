use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use garde::Report;
use sea_orm::DbErr;
use serde_json::json;
use thiserror::Error;
use tracing::error;

// 定义自定义错误类型
#[derive(Error, Debug)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    DB(#[from] DbErr),
    #[error("验证错误: {0}")]
    Validation(#[from] Report),
    #[error("{1}")]
    Api(StatusCode, String),
}

// 实现 IntoResponse trait，将 AppError 转换为统一 JSON 格式响应
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::DB(_) => {
                // 数据库错误不暴露内部细节，仅记录日志
                error!("数据库操作失败");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "服务器内部错误".to_string(),
                )
            }
            AppError::Validation(e) => {
                tracing::warn!("输入验证失败");
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "code": 400,
                        "message": "输入验证失败",
                        "errors": e,
                        "data": null
                    })),
                )
                    .into_response();
            }
            AppError::Api(status, msg) => (status, msg),
        };
        error!("err info: {}", error_message);
        let body = Json(json!({
            "code": status.as_u16(),
            "message": error_message,
            "data": null
        }));
        (status, body).into_response()
    }
}

impl AppError {
    pub fn internal(msg: &str) -> Self {
        AppError::Api(StatusCode::INTERNAL_SERVER_ERROR, msg.to_string())
    }
}

impl From<auth_core::AuthError> for AppError {
    fn from(error: auth_core::AuthError) -> Self {
        use auth_core::AuthError;

        match error {
            AuthError::Database(error) => Self::DB(error),
            AuthError::PhoneAlreadyRegistered => {
                Self::Api(StatusCode::BAD_REQUEST, "手机号已注册".to_string())
            }
            AuthError::PasswordTooLong => Self::Api(
                StatusCode::BAD_REQUEST,
                "密码不能超过72个UTF-8字节".to_string(),
            ),
            AuthError::InvalidCredentials => {
                Self::Api(StatusCode::UNAUTHORIZED, "手机号或密码错误".to_string())
            }
            AuthError::UserNotFound => {
                Self::Api(StatusCode::UNAUTHORIZED, "用户不存在".to_string())
            }
            AuthError::LinkedAccountNotFound => {
                error!("OAuth 身份关联的本地账户不存在");
                Self::internal("服务器内部错误")
            }
            AuthError::UnsupportedOAuthProvider => Self::Api(
                StatusCode::BAD_REQUEST,
                "不支持的 OAuth2 提供商".to_string(),
            ),
            AuthError::OAuthProviderNotConfigured => Self::Api(
                StatusCode::SERVICE_UNAVAILABLE,
                "OAuth2 提供商未配置".to_string(),
            ),
            AuthError::InvalidOAuthState => Self::Api(
                StatusCode::BAD_REQUEST,
                "CSRF token 无效或已过期".to_string(),
            ),
            AuthError::InvalidOAuthProviderConfiguration => {
                error!("OAuth2 提供商配置无效");
                Self::internal("服务器内部错误")
            }
            AuthError::OAuthHttpClient
            | AuthError::OAuthCodeExchange
            | AuthError::OAuthProfileRequest
            | AuthError::OAuthProfileResponse => {
                error!("OAuth2 提供商认证流程失败");
                Self::Api(StatusCode::BAD_GATEWAY, "OAuth2 提供商认证失败".to_string())
            }
            AuthError::InvalidRefreshToken => Self::Api(
                StatusCode::UNAUTHORIZED,
                "Refresh Token 无效或已过期".to_string(),
            ),
            AuthError::RefreshTokenRevoked => {
                Self::Api(StatusCode::UNAUTHORIZED, "Refresh Token 已失效".to_string())
            }
            AuthError::Redis(error) => {
                error!("认证服务 Redis 操作失败: {}", error);
                Self::internal("服务器内部错误")
            }
            AuthError::PasswordHash(error) => {
                error!("密码哈希操作失败: {}", error);
                Self::internal("服务器内部错误")
            }
            AuthError::Token(error) => {
                error!("JWT 操作失败: {}", error);
                Self::internal("服务器内部错误")
            }
        }
    }
}

pub type ApiResult<T> = Result<Json<T>, AppError>;

/// 成功响应的辅助函数
pub fn ok<T>(data: T) -> ApiResult<T> {
    Ok(Json(data))
}

/// 启动阶段的致命错误
///
/// 由 `main` 返回 `Err` 结束进程：进程以非零状态码退出，且 `main` 返回后
/// logger guard 才会析构并刷新缓冲日志，比 `std::process::exit` 更可靠。
///
/// 与 [`AppError`] 刻意分开：`AppError` 面向 HTTP 请求并实现了 `IntoResponse`，
/// 而本类型只描述「进程无法启动」，不产生任何响应。
#[derive(Error, Debug)]
pub enum StartupError {
    /// 服务运行失败，例如监听 socket 出错
    #[error("服务启动失败: {0}")]
    Serve(#[from] std::io::Error),
    /// 初始化步骤失败，只保留可公开的说明
    /// （数据库 / Redis 连接的详细原因已单独写入日志，避免凭据进入 stderr）
    #[error("初始化失败: {0}")]
    Init(String),
    /// Invalid or incomplete application configuration
    #[error("配置错误: {0}")]
    Config(#[from] super::configs::ConfigError),
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;

    #[test]
    fn internal_error_has_correct_status() {
        let err = AppError::internal("测试错误");
        match err {
            AppError::Api(StatusCode::INTERNAL_SERVER_ERROR, msg) => {
                assert_eq!(msg, "测试错误");
            }
            _ => panic!("internal() 应返回 Api(500, ..)"),
        }
    }

    #[test]
    fn db_error_display() {
        let err = AppError::DB(sea_orm::DbErr::Custom("db failure".to_string()));
        let msg = format!("{}", err);
        assert!(
            msg.contains("db failure"),
            "Display should include db error: {}",
            msg
        );
    }

    #[test]
    fn ok_helper_returns_ok() {
        let result: ApiResult<i32> = ok(42);
        assert!(result.is_ok());
        let Json(val) = result.unwrap();
        assert_eq!(val, 42);
    }

    #[test]
    fn api_error_display() {
        let err = AppError::Api(StatusCode::NOT_FOUND, "资源不存在".to_string());
        let msg = format!("{}", err);
        assert_eq!(msg, "资源不存在");
    }

    #[test]
    fn from_validation_errors() {
        use garde::Validate;
        #[derive(Validate)]
        struct TestInput {
            #[garde(required)]
            pub name: Option<String>,
        }
        let input = TestInput { name: None };
        let err = AppError::from(input.validate().unwrap_err());
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn from_db_error() {
        let db_err = sea_orm::DbErr::Custom("conn failed".to_string());
        let err: AppError = db_err.into();
        assert!(matches!(err, AppError::DB(_)));
    }

    #[test]
    fn startup_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::AddrInUse, "地址已被占用");
        let err: StartupError = io_err.into();
        assert!(matches!(err, StartupError::Serve(_)));
        assert!(
            format!("{}", err).contains("服务启动失败"),
            "Display 应包含启动失败前缀: {}",
            err
        );
    }

    #[test]
    fn startup_error_init_display() {
        let err = StartupError::Init("数据库连接失败".to_string());
        assert_eq!(format!("{}", err), "初始化失败: 数据库连接失败");
    }
}
