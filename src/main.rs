use axum::{Router, middleware, routing::get};
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, info};
use utoipa_scalar::{Scalar, Servable};

use api_doc::ApiDoc;
use constants::{AppState, CONFIG};
use project::error::StartupError;
use project::middlewares::auth::auth_middleware;
use project::{db, fallback, logger, middlewares::response::redirect_response};

mod api_doc;
mod apps;
mod constants;
mod helper;
mod project;

#[tokio::main]
async fn main() -> Result<(), StartupError> {
    // 初始化日志系统，返回 guard 用于保持日志写入器存活
    // guard 必须保持到程序结束，否则日志可能未写入就丢失
    let _guard = logger::init(&CONFIG);

    // 初始化数据库连接（自动同步 Schema）
    let db = match db::init(&CONFIG.database).await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("数据库连接失败: {}", e);
            return Err(StartupError::Init("数据库连接失败".into()));
        }
    };

    // 初始化 Redis 连接
    let redis_conn = match db::init_redis(&CONFIG.redis).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Redis 连接失败: {}", e);
            return Err(StartupError::Init("Redis 连接失败".into()));
        }
    };

    // 请求日志中间件：记录每个请求和响应
    let trace = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    // 应用状态，通过 State 注入到 handler
    let app_state = AppState {
        db,
        redis: redis_conn,
    };

    // 中间件堆栈（自外向内执行）：
    // 1. redirect_response - 自定义响应处理
    // 2. auth_middleware - JWT 认证（公开路径自动跳过）
    // 3. CompressionLayer - 响应压缩
    // 4. trace - 请求日志
    let middleware_stack = ServiceBuilder::new()
        .layer(trace)
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(auth_middleware))
        .layer(middleware::from_fn(redirect_response));

    // OpenAPI 文档（utoipauto 自动收集，响应体自动包装为 JsonResponse）
    let openapi = ApiDoc::spec();

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .nest("/api/v1", apps::v1_routes())
        .merge(Scalar::with_url("/docs", openapi))
        .fallback(fallback)
        .layer(middleware_stack)
        .with_state(app_state);

    // 绑定监听地址
    let server_url = format!("{}:{}", CONFIG.app.host, CONFIG.app.port);
    let listener = match tokio::net::TcpListener::bind(&server_url).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("failed to bind {}: {}", server_url, e);
            return Err(StartupError::Init(format!("端口绑定失败: {server_url}")));
        }
    };

    match listener.local_addr() {
        Ok(addr) => info!("listening on http://{}", addr),
        Err(e) => tracing::warn!(
            "listening on {} (local_addr unavailable: {})",
            server_url,
            e
        ),
    }

    // 启动服务器，支持优雅关闭（收到信号后等待请求处理完成再退出）
    // 运行期错误直接上抛：`main` 返回 Err 即以非零状态码退出，而不是 panic
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// 监听关闭信号：Ctrl+C（本地开发）或 SIGTERM（Docker/系统 kill）
/// 收到信号后等待正在处理的请求完成，再安全关闭服务器
///
/// 注意：注册信号处理器失败时不能提前结束等待，否则会「误触发」优雅关闭。
/// 此时记录错误并改用 pending 占位，让服务继续运行，只依赖另一路信号。
async fn shutdown_signal() {
    // 监听 Ctrl+C（本地开发环境）
    let ctrl_c = async {
        // 返回 Err 说明控制台事件句柄安装失败，此时不应关闭服务
        if let Err(e) = signal::ctrl_c().await {
            tracing::error!("failed to install Ctrl+C handler: {}", e);
            // 安装失败则阻塞，不触发关闭（与下方 SIGTERM 分支保持一致）
            std::future::pending::<()>().await;
        }
    };

    // 监听 SIGTERM（Docker 容器收到 docker stop 时触发）
    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                let _ = s.recv().await;
            }
            Err(e) => {
                tracing::error!("failed to install SIGTERM handler: {}", e);
                // 如果安装失败则阻塞，不触发关闭
                std::future::pending::<()>().await;
            }
        }
    };

    // Windows 不支持 SIGTERM，使用 pending 占位
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    // 等待任意一个信号触发
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("shutdown signal received, stopping server...");
}
