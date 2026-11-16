use sea_orm::MockDatabase;
use sea_orm::MockExecResult;

/// 测试用 Entity，带软删除
mod test_entity {
    use macros::soft_delete;
    use sea_orm::entity::prelude::*;

    #[soft_delete]
    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "test_table")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
        pub create_ts: DateTime,
        pub update_ts: DateTime,
        pub delete_ts: Option<DateTime>,
    }
}

use test_entity as TestEntity;

/// 测试：默认 find() 过滤已删除记录
#[test]
fn test_find_filters_deleted() {
    let db = MockDatabase::new(sea_orm::DatabaseBackend::Postgres)
        .append_query_results([vec![TestEntity::Model {
            id: 1,
            name: "Alice".to_string(),
            create_ts: chrono::NaiveDateTime::default(),
            update_ts: chrono::NaiveDateTime::default(),
            delete_ts: None,
        }]])
        .into_connection();

    let _result = futures::executor::block_on(async { TestEntity::Entity::find().all(&db).await });

    // 验证 SQL 包含 delete_ts IS NULL
    let sql = db.into_transaction_log();
    assert_eq!(sql.len(), 1);
    let query = format!("{:?}", sql[0]);
    assert!(
        query.contains("delete_ts") && query.contains("NULL"),
        "default find() should filter by delete_ts IS NULL, got: {}",
        query
    );
}

/// 测试：默认 find_by_id() 过滤已删除记录
#[test]
fn test_find_by_id_filters_deleted() {
    let db = MockDatabase::new(sea_orm::DatabaseBackend::Postgres)
        .append_query_results([vec![TestEntity::Model {
            id: 1,
            name: "Alice".to_string(),
            create_ts: chrono::NaiveDateTime::default(),
            update_ts: chrono::NaiveDateTime::default(),
            delete_ts: None,
        }]])
        .into_connection();

    let _result =
        futures::executor::block_on(async { TestEntity::Entity::find_by_id(1).all(&db).await });

    let sql = db.into_transaction_log();
    assert_eq!(sql.len(), 1);
    let query = format!("{:?}", sql[0]);
    assert!(
        query.contains("delete_ts") && query.contains("NULL"),
        "default find_by_id() should filter by delete_ts IS NULL, got: {}",
        query
    );
}

/// 测试：find_all() 不过滤已删除记录
#[test]
fn test_find_all_includes_deleted() {
    let db = MockDatabase::new(sea_orm::DatabaseBackend::Postgres)
        .append_query_results([vec![
            TestEntity::Model {
                id: 1,
                name: "Alice".to_string(),
                create_ts: chrono::NaiveDateTime::default(),
                update_ts: chrono::NaiveDateTime::default(),
                delete_ts: None,
            },
            TestEntity::Model {
                id: 2,
                name: "Bob".to_string(),
                create_ts: chrono::NaiveDateTime::default(),
                update_ts: chrono::NaiveDateTime::default(),
                delete_ts: Some(chrono::NaiveDateTime::default()),
            },
        ]])
        .into_connection();

    let result =
        futures::executor::block_on(async { TestEntity::Entity::find_all().all(&db).await })
            .unwrap();

    assert_eq!(
        result.len(),
        2,
        "find_all() should return all records including deleted"
    );

    let sql = db.into_transaction_log();
    let query = format!("{:?}", sql[0]);
    assert!(
        !query.contains("delete_ts") || !query.contains("IS NULL"),
        "find_all() should NOT filter by delete_ts IS NULL, got: {}",
        query
    );
}

/// 测试：默认 find() 显式过滤已删除记录
#[test]
fn test_soft_delete_query_filters_deleted_records() {
    let model = TestEntity::Model {
        id: 1,
        name: "Alice".to_string(),
        create_ts: chrono::NaiveDateTime::default(),
        update_ts: chrono::NaiveDateTime::default(),
        delete_ts: None,
    };

    let db = MockDatabase::new(sea_orm::DatabaseBackend::Postgres)
        .append_query_results([vec![model]])
        .into_connection();

    let _result = futures::executor::block_on(async { TestEntity::Entity::find().all(&db).await });

    let sql = db.into_transaction_log();
    let query = format!("{:?}", sql[0]);
    assert!(
        query.contains("delete_ts") && query.contains("IS NULL"),
        "default find() should filter soft deleted records, got: {}",
        query
    );
}

/// 测试：软删除只更新一次，并同步更新时间戳
#[test]
fn test_soft_delete_updates_timestamps_once() {
    let db = MockDatabase::new(sea_orm::DatabaseBackend::Postgres)
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }])
        .into_connection();

    let model = TestEntity::Model {
        id: 1,
        name: "Alice".to_string(),
        create_ts: chrono::NaiveDateTime::default(),
        update_ts: chrono::NaiveDateTime::default(),
        delete_ts: None,
    };

    let result = futures::executor::block_on(async { model.delete(&db).await }).unwrap();
    assert_eq!(result.rows_affected, 1);

    let sql = db.into_transaction_log();
    assert_eq!(sql.len(), 1);
    let query = format!("{:?}", sql[0]);
    assert!(
        query.contains("UPDATE"),
        "expected UPDATE query, got: {query}"
    );
    assert!(
        query.contains("update_ts"),
        "soft delete should update update_ts, got: {query}"
    );
    assert!(
        query.contains("delete_ts") && query.contains("NULL"),
        "soft delete should only affect active rows, got: {query}"
    );
}

/// 测试：hard_delete() 执行真正的 DELETE
#[test]
fn test_hard_delete_is_real_delete() {
    let db = MockDatabase::new(sea_orm::DatabaseBackend::Postgres)
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }])
        .into_connection();

    let model = TestEntity::Model {
        id: 1,
        name: "Alice".to_string(),
        create_ts: chrono::NaiveDateTime::default(),
        update_ts: chrono::NaiveDateTime::default(),
        delete_ts: None,
    };

    let result = futures::executor::block_on(async { model.hard_delete(&db).await }).unwrap();

    assert_eq!(result.rows_affected, 1);

    let sql = db.into_transaction_log();
    assert_eq!(sql.len(), 1);
    let query = format!("{:?}", sql[0]);
    assert!(
        query.contains("DELETE") || query.contains("delete"),
        "hard_delete() should be real DELETE, got: {}",
        query
    );
}
