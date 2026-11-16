# Web-Demo

一个基于 Axum + SeaORM 的 CRUD Demo 项目。

## 技术栈

| 组件 | 技术 |
|------|------|
| Web 框架 | Axum 0.8 |
| ORM | SeaORM 2.0 (SQLite / MySQL / PostgreSQL) |
| 缓存 | Redis (验证码存储、频率限制) |
| 异步运行时 | Tokio |
| 序列化 | Serde |
| 字段校验 | Validator |
| 认证 | JWT (access + refresh 双 token) |
| 密码/哈希 | bcrypt |
| OpenAPI 文档 | utoipa + utoipauto (自动收集) |
| 文档 UI | Scalar |

## 项目结构

```
src/
├── main.rs                 # 入口：服务器启动、中间件、路由挂载
├── api_doc.rs              # OpenAPI 文档定义（utoipauto 自动扫描）
├── constants.rs            # 全局常量（AppState、CONFIG）
├── helper/
│   ├── mod.rs
│   └── tools.rs            # 工具函数（日期格式化等）
├── apps/
│   ├── mod.rs              # 路由注册
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── view.rs         # 认证接口（发验证码、注册、登录、刷新 token）
│   │   ├── schemas.rs      # 认证请求/响应结构体
│   └── user/
│       ├── mod.rs
│       ├── view.rs         # 用户接口（CRUD）
│       ├── schemas.rs      # 用户请求/响应结构体
│       └── constants.rs    # 用户常量（枚举类型）
└── project/
    ├── mod.rs
    ├── configs.rs          # 配置加载（Jwt、Redis 等）
    ├── db.rs               # 数据库 + Redis 连接初始化
    ├── error.rs            # 统一错误处理
    ├── extractor.rs        # 自定义提取器（ResourceId）
    ├── logger.rs           # 日志系统
    ├── pagination.rs       # 分页结构体
    ├── redis.rs            # Redis 工具（频率限制宏）
    ├── response.rs         # 统一响应格式
    └── middlewares/
        ├── mod.rs
        ├── auth.rs         # JWT 认证中间件
        └── response.rs     # 响应中间件

auth/
└── src/
    ├── service.rs          # 注册、登录、Token 刷新
    ├── token.rs            # JWT 签发与校验
    ├── password.rs         # 密码哈希与校验
    ├── oauth.rs            # OAuth 通用协调入口
    └── oauth/
        ├── config.rs       # provider 配置与选择
        ├── account.rs      # 身份关联与本地账户事务
        ├── state.rs        # CSRF state 的 Redis 存取
        ├── http.rs         # 共用 HTTP 客户端及 JSON 请求
        └── providers/      # GitHub、Google、Microsoft 策略
```

## API 文档

项目使用 [utoipauto](https://crates.io/crates/utoipauto) 自动扫描 `#[utoipa::path]` 注解，无需手动注册路由。

启动后访问 Scalar UI：

```
http://localhost:8000/docs
```

### OpenAPI 规范说明

- `#[derive(IntoParams)]` 默认将参数标记为 `in = "path"`
- Query 参数需显式指定：`#[into_params(parameter_in = Query)]`
- `DateTimeLocal` 等自定义类型需用 `#[schema(value_type = String)]` 指定 OpenAPI 类型
- 所有响应体自动包装为 `{ code, message, data }` 结构（通过 `ApiDoc::spec()` 后处理）
- 全局 Bearer Auth 通过 `JwtSecurityAddon`（`Modify` trait）添加
- 公开接口使用 `security(())` 覆盖全局认证要求

## 用户管理权限

`GET /api/v1/user/list` 和 `POST /api/v1/user` 仅允许内部员工账号调用；用户详情、修改和删除接口只操作当前登录账号。项目没有公开的员工提权接口：首次员工账号需先正常注册，再由受信任的运维人员将对应账户的 `type` 更新为 `2`（内部员工），不能通过普通用户注册获得员工权限。

短信验证码目前仅支持开发环境日志演示；生产环境未配置短信供应商时，发送接口会明确返回 `503`，不会伪装成发送成功。验证码最多允许五次错误尝试，成功校验后即失效。

应用配置统一从 `config.toml` 读取，包括数据库连接串、JWT 密钥、Redis 密码及 OAuth client credentials。可复制 `config.toml.example` 后填写本地配置；JWT 两个密钥必须分别使用至少 32 字节的随机值。该文件已加入 `.gitignore`，不要提交真实密钥或凭据。Docker 镜像不包含本机 `config.toml`、数据库或日志文件，部署时需挂载或以其他安全方式提供配置文件。

OAuth2 provider 配置目前支持 `github`、`google` 和 `microsoft` 三个渠道。各渠道使用独立的 `client_id`、`client_secret`、授权端点、回调地址和 scopes；provider 用户资料获取及字段映射由 `auth-core` 负责。

数据库启动时会创建活跃账户手机号及活跃 OAuth 提供商用户 ID 的唯一索引（软删除记录不占用唯一值）。如果既有数据库存在重复的活跃值，应用会在初始化失败；部署前需先清理冲突数据。

## 运行

```bash
# 复制 config.toml.example 为 config.toml 并按需填写
cargo run
```

依赖版本已升级至最新可用版本。`.cargo/config.toml` 设置依赖至少发布满 7 天后才参与解析和更新；此配置需要 Cargo 1.100 或更新版本。
