use entity::{
    account::{ActiveModel as AccountActiveModel, Column as AccountColumn},
    oauth_account::{ActiveModel as OauthAccountActiveModel, Column as OauthAccountColumn},
    prelude::{Account, OauthAccount},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, IntoActiveModel, ModelTrait, QueryFilter,
};

use super::{OAuthUserInfo, config::OAuthProvider};
use crate::AuthError;

pub async fn link_or_create_account(
    db: &sea_orm::DbConn,
    provider: &str,
    user_info: &OAuthUserInfo,
) -> Result<entity::account::Model, AuthError> {
    let provider = OAuthProvider::parse(provider)?;
    match db
        .transaction_async(async |txn| {
            let existing_oauth = OauthAccount::find()
                .filter(OauthAccountColumn::Provider.eq(provider.as_str()))
                .filter(OauthAccountColumn::ProviderUserId.eq(&user_info.provider_user_id))
                .one(txn)
                .await?;

            if let Some(oauth) = existing_oauth {
                let user = oauth
                    .find_related(Account)
                    .filter(AccountColumn::DeleteTs.is_null())
                    .one(txn)
                    .await?
                    .ok_or(AuthError::LinkedAccountNotFound)?;

                let mut account_active = user.clone().into_active_model();
                account_active.last_login_dt = Set(Some(chrono::Utc::now().naive_utc()));
                if let Some(email) = &user_info.email {
                    account_active.email = Set(Some(email.clone()));
                }
                account_active.update(txn).await?;

                let mut oauth_active = oauth.into_active_model();
                if let Some(email) = &user_info.email {
                    oauth_active.email = Set(Some(email.clone()));
                }
                oauth_active.nickname = Set(Some(user_info.nickname.clone()));
                oauth_active.avatar_url = Set(user_info.avatar_url.clone());
                oauth_active.update(txn).await?;

                Account::find()
                    .filter(AccountColumn::Id.eq(user.id))
                    .one(txn)
                    .await?
                    .ok_or(AuthError::LinkedAccountNotFound)
            } else {
                let account = AccountActiveModel {
                    uid: Set(uuid::Uuid::new_v4().to_string()),
                    nickname: Set(user_info.nickname.clone()),
                    password: Set(String::new()),
                    name: Set(None),
                    telephone: Set(None),
                    email: Set(user_info.email.clone()),
                    r#type: Set(1),
                    last_login_dt: Set(Some(chrono::Utc::now().naive_utc())),
                    ..Default::default()
                }
                .insert(txn)
                .await?;

                OauthAccountActiveModel {
                    account_id: Set(account.id),
                    provider: Set(provider.as_str().to_string()),
                    provider_user_id: Set(user_info.provider_user_id.clone()),
                    email: Set(user_info.email.clone()),
                    nickname: Set(Some(user_info.nickname.clone())),
                    avatar_url: Set(user_info.avatar_url.clone()),
                    ..Default::default()
                }
                .insert(txn)
                .await?;

                Ok(account)
            }
        })
        .await
    {
        Ok(account) => Ok(account),
        Err(sea_orm::TransactionError::Connection(error)) => Err(AuthError::Database(error)),
        Err(sea_orm::TransactionError::Transaction(error)) => Err(error),
    }
}
