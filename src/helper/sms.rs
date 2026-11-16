use axum::http::StatusCode;
use rand::RngExt;
use tracing::info;

use crate::project::error::AppError;

const CODE_PREFIX: &str = "sms:code:";
const CODE_EXPIRE_SECONDS: u64 = 300;
const CODE_ATTEMPTS_PREFIX: &str = "sms:attempts:";
const MAX_CODE_ATTEMPTS: u64 = 5;
const LIMIT_PREFIX: &str = "sms:limit:";
const MIN_INTERVAL_SECONDS: u64 = 60;

/// 生成6位数字验证码
pub fn generate_code() -> String {
    rand::rng().random_range(100000..=999999).to_string()
}

/// 发送验证码（含频率限制、Redis 存储、日志输出）
pub async fn send_code(
    redis: &::redis::aio::ConnectionManager,
    phone: &str,
) -> Result<String, AppError> {
    if crate::constants::config().app.is_prod() {
        return Err(AppError::Api(
            StatusCode::SERVICE_UNAVAILABLE,
            "短信服务未配置".to_string(),
        ));
    }

    let limit_key = format!("{}{}", LIMIT_PREFIX, phone);
    if !crate::project::redis::set_nx_with_expire(redis, &limit_key, MIN_INTERVAL_SECONDS as i64)
        .await?
    {
        return Err(AppError::Api(
            StatusCode::TOO_MANY_REQUESTS,
            "发送过于频繁，请稍后再试".to_string(),
        ));
    }

    // 生成并存储验证码
    let code = generate_code();
    let code_key = format!("{}{}", CODE_PREFIX, phone);
    crate::project::redis::set_ex(redis, &code_key, &code, CODE_EXPIRE_SECONDS).await?;

    if crate::constants::config().app.is_prod() {
        info!("验证码已生成");
    } else {
        info!("验证码已发送 -> 手机号: {}, 验证码: {}", phone, code);
    }
    Ok(code)
}

/// 校验并消费验证码，最多允许五次错误尝试
pub async fn verify_code(
    redis: &::redis::aio::ConnectionManager,
    phone: &str,
    code: &str,
) -> Result<(), AppError> {
    let key = format!("{}{}", CODE_PREFIX, phone);
    let attempts_key = format!("{}{}", CODE_ATTEMPTS_PREFIX, phone);
    match crate::project::redis::verify_and_consume_code(
        redis,
        &key,
        &attempts_key,
        code,
        CODE_EXPIRE_SECONDS,
        MAX_CODE_ATTEMPTS,
    )
    .await?
    {
        1 => Ok(()),
        0 => Err(AppError::Api(
            StatusCode::BAD_REQUEST,
            "验证码错误".to_string(),
        )),
        _ => Err(AppError::Api(
            StatusCode::BAD_REQUEST,
            "验证码错误次数过多或已过期".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_code_length() {
        let code = generate_code();
        assert_eq!(code.len(), 6, "验证码应为6位数字");
    }

    #[test]
    fn generate_code_is_digits() {
        let code = generate_code();
        assert!(
            code.chars().all(|c| c.is_ascii_digit()),
            "验证码应全部为数字: {}",
            code
        );
    }

    #[test]
    fn generate_code_is_random() {
        let codes: std::collections::HashSet<String> = (0..100).map(|_| generate_code()).collect();
        assert!(
            codes.len() > 90,
            "100次生成应有足够不同的验证码，实际 {} 种",
            codes.len()
        );
    }

    #[test]
    fn generate_code_in_range() {
        for _ in 0..1000 {
            let code: u32 = generate_code().parse().unwrap();
            assert!(
                (100000..=999999).contains(&code),
                "验证码应在100000~999999之间: {}",
                code
            );
        }
    }
}
