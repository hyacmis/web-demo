use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// 发送验证码请求
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct SendSmsCodeInput {
    /// 手机号
    #[garde(custom(crate::helper::validation::validate_phone))]
    pub phone: String,
}

/// 注册请求
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct RegisterInput {
    /// 手机号
    #[garde(custom(crate::helper::validation::validate_phone))]
    pub phone: String,
    /// 验证码
    #[garde(length(equal = 6))]
    pub code: String,
    /// 密码
    #[garde(length(min = 6), custom(auth_core::validate_bcrypt_password))]
    pub password: String,
    /// 昵称
    #[garde(length(min = 1))]
    pub nickname: String,
    /// 姓名（可选）
    pub name: Option<String>,
}

/// 登录请求
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct LoginInput {
    /// 手机号
    #[garde(custom(crate::helper::validation::validate_phone))]
    pub phone: String,
    /// 密码
    #[garde(length(min = 6), custom(auth_core::validate_bcrypt_password))]
    pub password: String,
}

/// 登录响应
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LoginOutput {
    /// Access Token
    pub token: String,
    /// Access Token 过期时间（秒）
    pub expires_in: i64,
    /// Refresh Token
    pub refresh_token: String,
    /// Refresh Token 过期时间（秒）
    pub refresh_expires_in: i64,
}

impl From<auth_core::TokenPair> for LoginOutput {
    fn from(tokens: auth_core::TokenPair) -> Self {
        Self {
            token: tokens.token,
            expires_in: tokens.expires_in,
            refresh_token: tokens.refresh_token,
            refresh_expires_in: tokens.refresh_expires_in,
        }
    }
}

/// 刷新 Token 请求
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct RefreshInput {
    /// Refresh Token
    pub refresh_token: String,
}
