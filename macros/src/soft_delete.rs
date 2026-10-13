use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        #input

        impl #name {
            /// 删除（默认软删除）：设置 delete_ts
            pub async fn delete(
                &self,
                db: &impl sea_orm::ConnectionTrait,
            ) -> Result<sea_orm::UpdateResult, sea_orm::DbErr> {
                use sea_orm::sea_query::Expr;

                Entity::update_many()
                    .col_expr(
                        Column::DeleteTs,
                        Expr::val(chrono::Utc::now().naive_utc()),
                    )
                    .filter(Column::Id.eq(self.id))
                    .exec(db)
                    .await
            }

            /// 硬删除：真正从数据库移除记录
            pub async fn hard_delete(
                self,
                db: &impl sea_orm::ConnectionTrait,
            ) -> Result<sea_orm::DeleteResult, sea_orm::DbErr> {
                sea_orm::ModelTrait::delete(self, db).await
            }
        }

        impl Entity {
            /// 查询未删除的记录（默认过滤器）
            pub fn find() -> sea_orm::Select<Entity> {
                <Entity as sea_orm::EntityTrait>::find().filter(Column::DeleteTs.is_null())
            }

            /// 按 ID 查询未删除记录
            pub fn find_by_id(id: <<Entity as sea_orm::EntityTrait>::PrimaryKey as sea_orm::PrimaryKeyTrait>::ValueType) -> sea_orm::Select<Entity> {
                <Entity as sea_orm::EntityTrait>::find_by_id(id).filter(Column::DeleteTs.is_null())
            }

            /// 查询所有记录（包含已删除）
            pub fn find_all() -> sea_orm::Select<Entity> {
                <Entity as sea_orm::EntityTrait>::find()
            }
        }

        #[async_trait::async_trait]
        impl sea_orm::ActiveModelBehavior for ActiveModel {
            /// insert/update 前自动设置时间戳
            async fn before_save<C>(
                mut self,
                _db: &C,
                insert: bool,
            ) -> Result<Self, sea_orm::DbErr>
            where
                C: sea_orm::ConnectionTrait,
            {
                let now = chrono::Utc::now().naive_utc();
                if insert {
                    self.create_ts = sea_orm::ActiveValue::Set(now);
                }
                self.update_ts = sea_orm::ActiveValue::Set(now);
                Ok(self)
            }
        }
    };

    TokenStream::from(expanded)
}
