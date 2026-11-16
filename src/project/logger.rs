use std::str::FromStr;

use super::configs::Configs;
use chrono::Local;
use tracing::Level;
use tracing_appender::{non_blocking, non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{
    EnvFilter,
    fmt::{format::Writer, time::FormatTime},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

/// 自定义 Local 时间格式
struct LocalTimer;

impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%Y-%m-%d %H:%M:%S"))
    }
}

/// 初始化日志系统
/// - 开发环境：输出到 stdout，启用颜色和详细信息
/// - 生产环境：写入文件，关闭颜色、线程ID、目标以减少日志量
pub fn init(cfg: &Configs) -> Result<WorkerGuard, std::io::Error> {
    // 设置日志等级
    let level = Level::from_str(&cfg.log.level.to_ascii_uppercase())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;

    // 创建日志目录（生产环境）
    if cfg.app.is_prod() {
        std::fs::create_dir_all(&cfg.log.path)?;
    }

    // 生产环境 → 写入文件（按天滚动）
    // 开发环境 → 输出到 stdout
    let (writer, guard) = if cfg.app.is_prod() {
        // 使用tracing_appender，指定日志的输出目标位置
        // 参考: https://docs.rs/tracing-appender/latest/tracing_appender/index.html
        let file_appender = rolling::daily(&cfg.log.path, &cfg.log.filename);
        non_blocking(file_appender)
    } else {
        non_blocking(std::io::stdout())
    };

    // EnvFilter - 使用配置文件中的级别
    let filter = EnvFilter::new(level.as_str());

    // fmt layer（官方推荐）
    let mut fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_timer(LocalTimer) // 日期格式
        .with_file(false) // 显示源代码文件路径
        .with_line_number(true) // 显示源代码行号
        .compact() // 紧凑、单行格式
        // .pretty() // 漂亮、多行格式，适合本地开发调试
        // .json() // JSON 格式，适合生产环境日志收集
        // .flatten_event(true) // 为json展平事件元数据
        ;

    // 开发环境：启用颜色和详细信息
    // 生产环境：关闭颜色（避免日志文件包含 ANSI 转义码）、线程ID、目标，减少日志量
    if cfg.app.is_prod() {
        fmt_layer = fmt_layer
            .with_ansi(false) // 关闭颜色
            .with_thread_ids(false) // 关闭线程ID
            .with_target(false); // 关闭目标
    } else {
        fmt_layer = fmt_layer
            .with_ansi(true)
            .with_thread_ids(true)
            .with_target(true);
    }

    // registry + layer（官方示例风格）
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    Ok(guard)
}
