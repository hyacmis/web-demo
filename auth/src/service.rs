use std::future::Future;

use chrono::Utc;
use entity::{
    account::{ActiveModel as AccountActiveModel, Column as AccountColumn},
    prelude::Account,
};
use redis::aio::ConnectionManager;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, IntoActiveModel, QueryFilter};
use uuid::Uuid;

use crate::{AuthError, JwtConfig, TokenPair, generate_token_pair, hash_password, verify_password};

pub async fn register<E, F, Fut>(
    db: &sea_orm::DbConn,
    phone: &str,
    password: &str,
    nickname: String,
    name: Option<String>,
    verify_code: F,
) -> Result<(), E>
where
    E: From<AuthError>,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(), E>>,
{
    let existing_user = Account::find()
        .filter(AccountColumn::Telephone.eq(phone))
        .one(db)
        .await
        .map_err(|error| E::from(AuthError::Database(error)))?;
    if existing_user.is_some() {
        return Err(AuthError::PhoneAlreadyRegistered.into());
    }

    verify_code().await?;
    let hashed_password = hash_password(password).map_err(E::from)?;
    let model = AccountActiveModel {
        uid: Set(Uuid::new_v4().to_string()),
        nickname: Set(nickname),
        password: Set(hashed_password),
        name: Set(name),
        telephone: Set(Some(phone.to_string())),
        r#type: Set(1),
        ..Default::default()
    };
    model
        .insert(db)
        .await
        .map_err(|error| E::from(AuthError::Database(error)))?;
    Ok(())
}

pub async fn login(
    db: &sea_orm::DbConn,
    phone: &str,
    password: &str,
    config: &JwtConfig,
) -> Result<TokenPair, AuthError> {
    let user = Account::find()
        .filter(AccountColumn::Telephone.eq(phone))
        .one(db)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;

    if user.password.is_empty() || !verify_password(password, &user.password)? {
        return Err(AuthError::InvalidCredentials);
    }

    let mut active_model = user.clone().into_active_model();
    active_model.last_login_dt = Set(Some(Utc::now().naive_utc()));
    active_model.update(db).await?;
    generate_token_pair(user.id, config)
}

pub async fn refresh(
    db: &sea_orm::DbConn,
    redis: &ConnectionManager,
    refresh_token: &str,
    config: &JwtConfig,
) -> Result<TokenPair, AuthError> {
    let claims = crate::verify_refresh_token(refresh_token, config)
        .map_err(|_| AuthError::InvalidRefreshToken)?;
    let ttl = crate::refresh_token_ttl(claims.exp, Utc::now().timestamp())
        .and_then(|ttl| ttl.checked_add(1))
        .ok_or(AuthError::InvalidRefreshToken)?;

    let user = Account::find()
        .filter(AccountColumn::Id.eq(claims.sub))
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)?;
    let replacement_tokens = generate_token_pair(user.id, config)?;

    let mut conn = redis.clone();
    let key = format!("token:blacklist:{}", claims.jti);
    let was_set: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg("1")
        .arg("NX")
        .arg("EX")
        .arg(ttl)
        .query_async(&mut conn)
        .await?;
    if was_set.is_none() {
        return Err(AuthError::RefreshTokenRevoked);
    }

    Ok(replacement_tokens)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use entity::account::Model as AccountModel;
    use futures::executor::block_on;
    use sea_orm::{DatabaseBackend, MockDatabase};

    use crate::{verify_access_token, verify_refresh_token};

    use super::*;

    fn account(password: String) -> AccountModel {
        AccountModel {
            id: 7,
            uid: "account-uid".to_string(),
            r#type: 1,
            nickname: "test-user".to_string(),
            password,
            name: None,
            gender: 0,
            telephone: Some("13800138000".to_string()),
            email: None,
            address: None,
            last_login_dt: None,
            create_ts: chrono::NaiveDateTime::default(),
            update_ts: chrono::NaiveDateTime::default(),
            delete_ts: None,
        }
    }

    fn jwt_config() -> JwtConfig {
        JwtConfig {
            secret: "access-secret-for-tests-at-least-32-bytes".to_string(),
            expires_in_hours: 1,
            refresh_secret: "refresh-secret-for-tests-at-least-32-bytes".to_string(),
            refresh_expires_in_days: 7,
        }
    }

    #[test]
    fn register_rejects_existing_phone_without_consuming_code() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![account("stored-hash".to_string())]])
            .into_connection();
        let verification_called = Cell::new(false);

        let result = block_on(register::<AuthError, _, _>(
            &db,
            "13800138000",
            "password",
            "new-user".to_string(),
            None,
            || {
                verification_called.set(true);
                std::future::ready(Ok(()))
            },
        ));

        assert!(matches!(result, Err(AuthError::PhoneAlreadyRegistered)));
        assert!(!verification_called.get());
        assert_eq!(db.into_transaction_log().len(), 1);
    }

    #[test]
    fn register_consumes_verification_before_insert() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<AccountModel>::new()])
            .append_query_results([vec![account("inserted-hash".to_string())]])
            .into_connection();
        let verification_called = Cell::new(false);

        let result = block_on(register::<AuthError, _, _>(
            &db,
            "13800138000",
            "password",
            "new-user".to_string(),
            Some("Test User".to_string()),
            || {
                verification_called.set(true);
                std::future::ready(Ok(()))
            },
        ));

        assert!(result.is_ok());
        assert!(verification_called.get());
        assert_eq!(db.into_transaction_log().len(), 2);
    }

    #[test]
    fn register_does_not_insert_when_code_verification_fails() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<AccountModel>::new()])
            .into_connection();

        let result = block_on(register::<AuthError, _, _>(
            &db,
            "13800138000",
            "password",
            "new-user".to_string(),
            None,
            || std::future::ready(Err(AuthError::InvalidRefreshToken)),
        ));

        assert!(matches!(result, Err(AuthError::InvalidRefreshToken)));
        assert_eq!(db.into_transaction_log().len(), 1);
    }

    #[test]
    fn login_rejects_unknown_phone_and_oauth_only_account() {
        let unknown_db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<AccountModel>::new()])
            .into_connection();
        let result = block_on(login(&unknown_db, "13800138000", "password", &jwt_config()));
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));

        let oauth_db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![account(String::new())]])
            .into_connection();
        let result = block_on(login(&oauth_db, "13800138000", "password", &jwt_config()));
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
        assert_eq!(oauth_db.into_transaction_log().len(), 1);
    }

    #[test]
    fn login_rejects_wrong_password_without_updating_account() {
        let password_hash = hash_password("correct-password").unwrap();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![account(password_hash)]])
            .into_connection();

        let result = block_on(login(&db, "13800138000", "wrong-password", &jwt_config()));

        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
        assert_eq!(db.into_transaction_log().len(), 1);
    }

    #[test]
    fn login_returns_tokens_and_updates_last_login_on_valid_password() {
        let password_hash = hash_password("correct-password").unwrap();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([
                vec![account(password_hash.clone())],
                vec![account(password_hash)],
            ])
            .into_connection();

        let tokens =
            block_on(login(&db, "13800138000", "correct-password", &jwt_config())).unwrap();

        assert_eq!(
            verify_access_token(&tokens.token, &jwt_config())
                .unwrap()
                .sub,
            7
        );
        assert_eq!(
            verify_refresh_token(&tokens.refresh_token, &jwt_config())
                .unwrap()
                .sub,
            7
        );
        assert_eq!(db.into_transaction_log().len(), 2);
    }
}
