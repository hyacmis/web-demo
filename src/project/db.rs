use std::time::Duration;

use redis::RedisResult;
use redis::aio::ConnectionManager;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DbConn};
use tracing::info;

use super::configs::{Database as cfg_database, Redis as cfg_redis};

/// 初始化数据库连接池并同步 Schema
pub async fn init(db: &cfg_database) -> Result<DbConn, sea_orm::DbErr> {
    let mut opt = ConnectOptions::new(db.url.expose_secret().to_string());
    opt.min_connections(2)
        .max_connections(10)
        .connect_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(3600))
        .sqlx_logging(db.debug);

    let conn = Database::connect(opt).await?;
    conn.ping().await?;

    // 自动同步 Entity 定义到数据库表结构
    conn.get_schema_registry("entity::*").sync(&conn).await?;
    ensure_unique_indexes(&conn).await?;

    info!("数据库初始化完成");
    Ok(conn)
}

async fn ensure_unique_indexes(conn: &impl ConnectionTrait) -> Result<(), sea_orm::DbErr> {
    conn.execute_unprepared(
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_account_active_telephone \
         ON account (telephone) WHERE telephone IS NOT NULL AND delete_ts IS NULL",
    )
    .await?;
    conn.execute_unprepared(
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_oauth_active_provider_user \
         ON oauth_account (provider, provider_user_id) WHERE delete_ts IS NULL",
    )
    .await?;
    Ok(())
}

/// 初始化 Redis 连接
pub async fn init_redis(redis: &cfg_redis) -> RedisResult<ConnectionManager> {
    // 创建客户端（内部管理连接池）
    let client = redis::Client::open(redis.url())?;

    // 创建连接管理器（自动管理连接池）
    let conn = ConnectionManager::new(client).await?;

    info!("Redis 连接初始化完成");
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn syncs_entity_schema_with_declared_relations() {
        let conn = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite memory database should connect");

        conn.get_schema_registry("entity::*")
            .sync(&conn)
            .await
            .expect("SeaORM entity schema and relations should sync");
    }

    #[tokio::test]
    async fn creates_active_account_and_oauth_uniqueness_indexes() {
        let conn = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite memory database should connect");
        conn.execute_unprepared("CREATE TABLE account (telephone TEXT, delete_ts TEXT)")
            .await
            .expect("account table should be created");
        conn.execute_unprepared(
            "CREATE TABLE oauth_account (provider TEXT, provider_user_id TEXT, delete_ts TEXT)",
        )
        .await
        .expect("oauth_account table should be created");

        ensure_unique_indexes(&conn)
            .await
            .expect("indexes should be created");
        ensure_unique_indexes(&conn)
            .await
            .expect("index creation should be idempotent");

        conn.execute_unprepared(
            "INSERT INTO account (telephone, delete_ts) VALUES ('13800138000', NULL)",
        )
        .await
        .expect("first active account phone should be accepted");
        assert!(
            conn.execute_unprepared(
                "INSERT INTO account (telephone, delete_ts) VALUES ('13800138000', NULL)",
            )
            .await
            .is_err()
        );
        conn.execute_unprepared(
            "INSERT INTO account (telephone, delete_ts) VALUES ('13800138000', '2025-01-01')",
        )
        .await
        .expect("deleted accounts should not reserve phone numbers");

        conn.execute_unprepared(
            "INSERT INTO oauth_account (provider, provider_user_id, delete_ts) \
             VALUES ('github', '123', NULL)",
        )
        .await
        .expect("first active OAuth account should be accepted");
        assert!(
            conn.execute_unprepared(
                "INSERT INTO oauth_account (provider, provider_user_id, delete_ts) \
                 VALUES ('github', '123', NULL)",
            )
            .await
            .is_err()
        );
    }
}
