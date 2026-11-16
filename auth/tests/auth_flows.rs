use auth_core::{AuthError, JwtConfig, login, register, verify_access_token, verify_refresh_token};
use entity::account::Model as AccountModel;
use futures::executor::block_on;
use sea_orm::{DatabaseBackend, MockDatabase};

fn account(password: String) -> AccountModel {
    AccountModel {
        id: 7,
        uid: "account-uid".to_string(),
        r#type: 1,
        nickname: "test-user".to_string(),
        password,
        name: Some("Test User".to_string()),
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
fn public_registration_flow_checks_verification_and_persists_account() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<AccountModel>::new()])
        .append_query_results([vec![account("registered-password-hash".to_string())]])
        .into_connection();
    let mut verification_checked = false;

    block_on(register::<AuthError, _, _>(
        &db,
        "13800138000",
        "correct-password",
        "test-user".to_string(),
        Some("Test User".to_string()),
        || {
            verification_checked = true;
            std::future::ready(Ok(()))
        },
    ))
    .expect("registration should succeed");

    assert!(verification_checked);
    assert_eq!(db.into_transaction_log().len(), 2);
}

#[test]
fn public_login_flow_returns_verifiable_access_and_refresh_tokens() {
    let password_hash = bcrypt::hash("correct-password", bcrypt::DEFAULT_COST).unwrap();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([
            vec![account(password_hash.clone())],
            vec![account(password_hash)],
        ])
        .into_connection();
    let config = jwt_config();

    let tokens = block_on(login(&db, "13800138000", "correct-password", &config))
        .expect("login should succeed");

    assert_eq!(verify_access_token(&tokens.token, &config).unwrap().sub, 7);
    assert_eq!(
        verify_refresh_token(&tokens.refresh_token, &config)
            .unwrap()
            .sub,
        7
    );
    assert_eq!(db.into_transaction_log().len(), 2);
}
