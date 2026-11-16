use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AuthError;

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub expires_in_hours: u64,
    pub refresh_secret: String,
    pub refresh_expires_in_days: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub jti: String,
    pub sub: i32,
    pub iat: i64,
    pub exp: i64,
    #[serde(rename = "type")]
    pub typ: String,
}

#[derive(Debug, Clone)]
pub struct TokenPair {
    pub token: String,
    pub expires_in: i64,
    pub refresh_token: String,
    pub refresh_expires_in: i64,
}

pub fn refresh_token_ttl(exp: i64, now: i64) -> Option<i64> {
    exp.checked_sub(now).filter(|ttl| *ttl > 0)
}

pub fn generate_token_pair(user_id: i32, config: &JwtConfig) -> Result<TokenPair, AuthError> {
    let now = Utc::now();
    let access_exp = now + Duration::hours(config.expires_in_hours as i64);
    let refresh_exp = now + Duration::days(config.refresh_expires_in_days as i64);
    let access_claims = Claims {
        jti: Uuid::new_v4().to_string(),
        sub: user_id,
        iat: now.timestamp(),
        exp: access_exp.timestamp(),
        typ: "access".to_string(),
    };
    let refresh_claims = Claims {
        jti: Uuid::new_v4().to_string(),
        sub: user_id,
        iat: now.timestamp(),
        exp: refresh_exp.timestamp(),
        typ: "refresh".to_string(),
    };

    Ok(TokenPair {
        token: encode(
            &Header::default(),
            &access_claims,
            &EncodingKey::from_secret(config.secret.as_bytes()),
        )?,
        expires_in: (config.expires_in_hours as i64) * 3600,
        refresh_token: encode(
            &Header::default(),
            &refresh_claims,
            &EncodingKey::from_secret(config.refresh_secret.as_bytes()),
        )?,
        refresh_expires_in: (config.refresh_expires_in_days as i64) * 86400,
    })
}

pub fn verify_access_token(token: &str, config: &JwtConfig) -> Result<Claims, AuthError> {
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.secret.as_bytes()),
        &Validation::default(),
    )?
    .claims;
    if claims.typ != "access" {
        return Err(jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::InvalidToken,
        )
        .into());
    }
    Ok(claims)
}

pub fn verify_refresh_token(token: &str, config: &JwtConfig) -> Result<Claims, AuthError> {
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.refresh_secret.as_bytes()),
        &Validation::default(),
    )?
    .claims;
    if claims.typ != "refresh" {
        return Err(jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::InvalidToken,
        )
        .into());
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> JwtConfig {
        JwtConfig {
            secret: "access-secret-for-tests-at-least-32-bytes".to_string(),
            expires_in_hours: 1,
            refresh_secret: "refresh-secret-for-tests-at-least-32-bytes".to_string(),
            refresh_expires_in_days: 7,
        }
    }

    #[test]
    fn generated_tokens_verify_only_for_their_token_type() {
        let config = config();
        let pair = generate_token_pair(42, &config).unwrap();

        assert_eq!(verify_access_token(&pair.token, &config).unwrap().sub, 42);
        assert_eq!(
            verify_refresh_token(&pair.refresh_token, &config)
                .unwrap()
                .sub,
            42
        );
        assert!(verify_refresh_token(&pair.token, &config).is_err());
        assert!(verify_access_token(&pair.refresh_token, &config).is_err());
    }

    #[test]
    fn refresh_token_must_not_be_expired_or_expiring_now() {
        assert_eq!(refresh_token_ttl(101, 100), Some(1));
        assert_eq!(refresh_token_ttl(100, 100), None);
        assert_eq!(refresh_token_ttl(99, 100), None);
    }

    #[test]
    fn refresh_token_ttl_rejects_overflow() {
        assert_eq!(refresh_token_ttl(i64::MAX, i64::MIN), None);
    }
}
