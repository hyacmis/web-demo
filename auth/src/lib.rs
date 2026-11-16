mod oauth;
mod password;
mod secret;
mod service;
mod token;

pub use oauth::{
    OAuthAuthorization, OAuthProvider, OAuthUserInfo, Oauth2, Oauth2Provider, consume_oauth_state,
    exchange_oauth_code, link_or_create_account, oauth_authorization, store_oauth_state,
};
pub use password::{hash_password, validate_bcrypt_password, verify_password};
pub use secret::SecretString;
pub use service::{login, refresh, register};
pub use token::{
    Claims, JwtConfig, TokenPair, generate_token_pair, refresh_token_ttl, verify_access_token,
    verify_refresh_token,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("database operation failed")]
    Database(#[from] sea_orm::DbErr),
    #[error("Redis operation failed")]
    Redis(#[from] redis::RedisError),
    #[error("password hashing failed")]
    PasswordHash(#[from] bcrypt::BcryptError),
    #[error("password exceeds the bcrypt input limit")]
    PasswordTooLong,
    #[error("token operation failed")]
    Token(#[from] jsonwebtoken::errors::Error),
    #[error("phone number is already registered")]
    PhoneAlreadyRegistered,
    #[error("invalid phone number or password")]
    InvalidCredentials,
    #[error("account does not exist")]
    UserNotFound,
    #[error("refresh token is invalid or expired")]
    InvalidRefreshToken,
    #[error("refresh token has already been used")]
    RefreshTokenRevoked,
    #[error("OAuth identity is linked to a missing account")]
    LinkedAccountNotFound,
    #[error("unsupported OAuth provider")]
    UnsupportedOAuthProvider,
    #[error("OAuth provider is not configured")]
    OAuthProviderNotConfigured,
    #[error("OAuth provider configuration is invalid")]
    InvalidOAuthProviderConfiguration,
    #[error("OAuth HTTP client initialization failed")]
    OAuthHttpClient,
    #[error("OAuth authorization code exchange failed")]
    OAuthCodeExchange,
    #[error("OAuth provider profile request failed")]
    OAuthProfileRequest,
    #[error("OAuth provider profile response is invalid")]
    OAuthProfileResponse,
    #[error("OAuth CSRF state is invalid or expired")]
    InvalidOAuthState,
}
