use auth_core::{OAuthUserInfo, link_or_create_account};
use entity::{account::Model as AccountModel, oauth_account::Model as OauthAccountModel};
use futures::executor::block_on;
use sea_orm::{DatabaseBackend, MockDatabase};

fn account() -> AccountModel {
    AccountModel {
        id: 8,
        uid: "new-account".to_string(),
        r#type: 1,
        nickname: "OAuth User".to_string(),
        password: String::new(),
        name: None,
        gender: 0,
        telephone: None,
        email: Some("user@example.com".to_string()),
        address: None,
        last_login_dt: None,
        create_ts: chrono::NaiveDateTime::default(),
        update_ts: chrono::NaiveDateTime::default(),
        delete_ts: None,
    }
}

fn oauth_account() -> OauthAccountModel {
    OauthAccountModel {
        id: 5,
        account_id: 8,
        provider: "github".to_string(),
        provider_user_id: "provider-user-1".to_string(),
        email: Some("user@example.com".to_string()),
        nickname: Some("OAuth User".to_string()),
        avatar_url: None,
        create_ts: chrono::NaiveDateTime::default(),
        update_ts: chrono::NaiveDateTime::default(),
        delete_ts: None,
    }
}

fn user_info() -> OAuthUserInfo {
    OAuthUserInfo {
        provider_user_id: "provider-user-1".to_string(),
        email: Some("user@example.com".to_string()),
        nickname: "OAuth User".to_string(),
        avatar_url: None,
    }
}

#[test]
fn oauth_login_creates_and_links_account_atomically() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<OauthAccountModel>::new()])
        .append_query_results([vec![account()]])
        .append_query_results([vec![oauth_account()]])
        .into_connection();

    let user = block_on(link_or_create_account(&db, "github", &user_info()))
        .expect("OAuth account creation should succeed");

    assert_eq!(user.id, 8);
    let transaction_log = db.into_transaction_log();
    assert_eq!(transaction_log.len(), 1);
    let queries = format!("{:?}", transaction_log[0]);
    assert!(queries.contains("oauth_account"));
    assert!(queries.contains("account"));
}

#[test]
fn oauth_login_updates_existing_account_and_provider_profile() {
    let mut existing_account = account();
    existing_account.nickname = "Old Name".to_string();
    existing_account.email = Some("old@example.com".to_string());
    let mut existing_oauth = oauth_account();
    existing_oauth.nickname = Some("Old Name".to_string());
    existing_oauth.email = Some("old@example.com".to_string());

    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![existing_oauth.clone()]])
        .append_query_results([vec![existing_account.clone()]])
        .append_query_results([vec![existing_account.clone()]])
        .append_query_results([vec![existing_oauth]])
        .append_query_results([vec![existing_account.clone()]])
        .into_connection();

    let user = block_on(link_or_create_account(&db, "github", &user_info()))
        .expect("existing OAuth identity should sign in");

    assert_eq!(user.id, existing_account.id);
    let transaction_log = db.into_transaction_log();
    assert_eq!(transaction_log.len(), 1);
    let queries = format!("{:?}", transaction_log[0]);
    assert!(queries.contains("oauth_account"));
    assert!(queries.contains("account"));
    assert!(queries.contains("UPDATE"));
}

#[test]
fn oauth_account_link_rejects_unsupported_provider_before_database_access() {
    let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();

    let result = block_on(link_or_create_account(&db, "unknown", &user_info()));

    assert!(matches!(
        result,
        Err(auth_core::AuthError::UnsupportedOAuthProvider)
    ));
    assert!(db.into_transaction_log().is_empty());
}
