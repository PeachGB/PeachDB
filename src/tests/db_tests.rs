use super::helpers::{DbCleanup, unique_db_name};
use crate::db::Database;
use crate::dtypes::{Field, Key, Primitive};

#[tokio::test]
async fn db_set_get_persist_roundtrip() {
    let name = unique_db_name("peachdb_test");
    let _cleanup = DbCleanup { name: name.clone() };

    let db = Database::open(name.clone()).await.expect("open db");

    let key = Key::from("test-key");
    let field = Field::Primitive(Primitive::String("hello-world".to_string()));
    db.set(key.clone(), field.clone()).await.expect("set");

    match db.get(&key).await.expect("get") {
        Field::Primitive(Primitive::String(s)) => assert_eq!(s, "hello-world"),
        _ => panic!("unexpected variant"),
    }

    db.flush().await.expect("flush");
    drop(db);

    let db2 = Database::open(name.clone()).await.expect("reopen");
    match db2.get(&key).await.expect("get after reopen") {
        Field::Primitive(Primitive::String(s)) => assert_eq!(s, "hello-world"),
        _ => panic!("unexpected variant after reopen"),
    }
}

#[tokio::test]
async fn db_delete_and_flush() {
    let name = unique_db_name("peachdb_test");
    let _cleanup = DbCleanup { name: name.clone() };

    let db = Database::open(name.clone()).await.expect("open db");

    let key = Key::from("to-delete");
    let field = Field::Primitive(Primitive::Integer(123));
    db.set(key.clone(), field).await.expect("set");
    db.flush().await.expect("flush");

    db.delete(key.clone()).await.expect("delete");
    db.flush().await.expect("flush after delete");

    assert!(
        db.get(&key).await.is_err(),
        "deleted key should not be found"
    );
}

// Regression test for bug 1: reopening a DB with tombstones used to fail with
// CorruptedRecord because check_crc ran on deleted records.
#[tokio::test]
async fn db_delete_survives_reopen() {
    let name = unique_db_name("peachdb_test");
    let _cleanup = DbCleanup { name: name.clone() };

    let db = Database::open(name.clone()).await.expect("open db");
    let key = Key::from("ephemeral");
    db.set(key.clone(), Field::Primitive(Primitive::Integer(1)))
        .await
        .expect("set");
    db.flush().await.expect("flush");
    db.delete(key.clone()).await.expect("delete");
    db.flush().await.expect("flush after delete");
    drop(db);

    let db2 = Database::open(name.clone()).await.expect("reopen after delete");
    assert!(db2.get(&key).await.is_err(), "deleted key must not reappear after reopen");
}

// Regression test for bug 3: Set→Delete in the same flush batch left an
// un-tombstoned record on disk that caused the key to reappear on reopen.
#[tokio::test]
async fn db_set_then_delete_same_flush_no_reappear() {
    let name = unique_db_name("peachdb_test");
    let _cleanup = DbCleanup { name: name.clone() };

    let db = Database::open(name.clone()).await.expect("open db");
    let key = Key::from("ghost");

    // First flush: establish the key on disk.
    db.set(key.clone(), Field::Primitive(Primitive::Integer(1)))
        .await
        .expect("set v1");
    db.flush().await.expect("flush v1");

    // Second flush: overwrite then immediately delete, all in one batch.
    db.set(key.clone(), Field::Primitive(Primitive::Integer(2)))
        .await
        .expect("set v2");
    db.delete(key.clone()).await.expect("delete");
    db.flush().await.expect("flush set+delete");
    drop(db);

    let db2 = Database::open(name.clone()).await.expect("reopen");
    assert!(db2.get(&key).await.is_err(), "key deleted in same flush must not reappear after reopen");
}
