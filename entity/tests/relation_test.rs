use entity::{
    account::{Entity as Account, Model as AccountModel},
    oauth_account::ModelEx as OauthAccountModel,
};
use futures::executor::block_on;
use sea_orm::{DatabaseBackend, MockDatabase, ModelTrait};

#[test]
fn oauth_account_can_load_its_related_local_account() {
    let account = AccountModel {
        id: 7,
        uid: "account-uid".to_string(),
        r#type: 1,
        nickname: "test-user".to_string(),
        password: String::new(),
        name: None,
        gender: 0,
        telephone: None,
        email: None,
        address: None,
        last_login_dt: None,
        create_ts: chrono::NaiveDateTime::default(),
        update_ts: chrono::NaiveDateTime::default(),
        delete_ts: None,
    };
    let oauth = OauthAccountModel {
        id: 3,
        account_id: account.id,
        account: Default::default(),
        provider: "github".to_string(),
        provider_user_id: "provider-user-id".to_string(),
        email: None,
        nickname: None,
        avatar_url: None,
        create_ts: chrono::NaiveDateTime::default(),
        update_ts: chrono::NaiveDateTime::default(),
        delete_ts: None,
    };
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![account.clone()]])
        .into_connection();

    let related = block_on(oauth.find_related(Account).all(&db)).unwrap();

    assert_eq!(related, vec![account]);
    assert_eq!(db.into_transaction_log().len(), 1);
}
