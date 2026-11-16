use crate::project::error::AppError;

/// Redis SETEX 辅助函数
pub async fn set_ex(
    redis: &redis::aio::ConnectionManager,
    key: &str,
    value: &str,
    ttl: u64,
) -> Result<(), AppError> {
    use redis::AsyncCommands;
    let mut conn = redis.clone();
    conn.set_ex(key, value, ttl).await.map_err(|e| {
        tracing::error!("Redis SETEX 操作失败: {}", e);
        AppError::internal("服务器内部错误")
    })
}

/// Redis GET 辅助函数
#[allow(dead_code)]
pub async fn get(
    redis: &redis::aio::ConnectionManager,
    key: &str,
) -> Result<Option<String>, AppError> {
    use redis::AsyncCommands;
    let mut conn = redis.clone();
    conn.get(key).await.map_err(|e| {
        tracing::error!("Redis GET 操作失败: {}", e);
        AppError::internal("服务器内部错误")
    })
}

/// Atomically verify and consume an SMS code, limiting incorrect attempts.
pub async fn verify_and_consume_code(
    redis: &redis::aio::ConnectionManager,
    code_key: &str,
    attempts_key: &str,
    code: &str,
    ttl: u64,
    max_attempts: u64,
) -> Result<i32, AppError> {
    let mut conn = redis.clone();
    redis::Script::new(
        "local stored = redis.call('GET', KEYS[1]); \
         if not stored then return -1 end; \
         if stored == ARGV[1] then \
             redis.call('DEL', KEYS[1], KEYS[2]); \
             return 1; \
         end; \
         local attempts = redis.call('INCR', KEYS[2]); \
         if attempts == 1 then redis.call('EXPIRE', KEYS[2], ARGV[2]); end; \
         if attempts >= tonumber(ARGV[3]) then \
             redis.call('DEL', KEYS[1], KEYS[2]); \
             return -2; \
         end; \
         return 0;",
    )
    .key(code_key)
    .key(attempts_key)
    .arg(code)
    .arg(ttl)
    .arg(max_attempts)
    .invoke_async(&mut conn)
    .await
    .map_err(|e| {
        tracing::error!("Redis 验证码校验失败: {}", e);
        AppError::internal("服务器内部错误")
    })
}

/// Redis SET NX EX（原子频率限制）
/// 返回 true 表示首次设置（通过），false 表示已存在（频率限制）
pub async fn set_nx_with_expire(
    redis: &redis::aio::ConnectionManager,
    key: &str,
    ttl: i64,
) -> Result<bool, AppError> {
    let mut conn = redis.clone();
    let result: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg("1")
        .arg("NX")
        .arg("EX")
        .arg(ttl)
        .query_async(&mut conn)
        .await
        .map_err(|e| {
            tracing::error!("Redis SET NX EX 操作失败: {}", e);
            AppError::internal("服务器内部错误")
        })?;
    Ok(result.is_some())
}
