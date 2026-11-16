use crate::soft_delete;
use sea_orm::entity::prelude::*;

/// OAuth 第三方账户绑定表
/// 活跃绑定的联合唯一索引由数据库初始化时创建。
#[soft_delete]
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "oauth_account")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(comment = "关联的本地账户 ID")]
    pub account_id: i32,
    #[sea_orm(belongs_to, from = "account_id", to = "id")]
    pub account: BelongsTo<super::account::Entity>,
    #[sea_orm(comment = "提供商: github, google 等")]
    pub provider: String,
    #[sea_orm(comment = "提供商用户 ID")]
    pub provider_user_id: String,
    #[sea_orm(comment = "提供商邮箱")]
    pub email: Option<String>,
    #[sea_orm(comment = "提供商昵称")]
    pub nickname: Option<String>,
    #[sea_orm(comment = "提供商头像 URL")]
    pub avatar_url: Option<String>,
    #[sea_orm(default_expr = "Expr::current_timestamp()", comment = "创建时间")]
    pub create_ts: DateTime,
    #[sea_orm(default_expr = "Expr::current_timestamp()", comment = "更新时间")]
    pub update_ts: DateTime,
    #[sea_orm(comment = "删除时间")]
    pub delete_ts: Option<DateTime>,
}
