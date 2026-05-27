use crate::db::Database;
use crate::dtypes::{Field, Key, Primitive};
use tokio::fs;

#[tokio::test]
async fn db_set_get_persist_roundtrip() {
    // unique name per test run
    let name = format!("peachdb_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

    println!("TEST: opening db {}", name);
    // open DB
    let db = Database::open(name.clone()).await.expect("open db");
    println!("TEST: opened db");

    // set a value
    let key = Key::from("test-key");
    let field = Field::Primitive(Primitive::String("hello-world".to_string()));
    println!("TEST: setting key");
    db.set(key.clone(), field.clone()).await.expect("set");
    println!("TEST: set complete");

    // get it back
    let got = db.get(&key).await.expect("get");
    match got {
        Field::Primitive(Primitive::String(s)) => assert_eq!(s, "hello-world"),
        _ => panic!("unexpected variant"),
    }

    // flush to disk
    println!("TEST: flushing");
    db.flush().await.expect("flush");
    println!("TEST: flushed");

    // drop and reopen
    drop(db);
    println!("TEST: reopening");
    let db2 = Database::open(name.clone()).await.expect("reopen");
    println!("TEST: reopened");
    let got2 = db2.get(&key).await.expect("get after reopen");
    match got2 {
        Field::Primitive(Primitive::String(s)) => assert_eq!(s, "hello-world"),
        _ => panic!("unexpected variant after reopen"),
    }

    // cleanup files
    let _ = fs::remove_file(format!("{}.db", name)).await;
    let _ = fs::remove_file(format!("{}.dbidx", name)).await;
    let _ = fs::remove_file(format!("{}.wal", name)).await;
}

#[tokio::test]
async fn db_delete_and_flush() {
    let name = format!("peachdb_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    let db = Database::open(name.clone()).await.expect("open db");

    let key = Key::from("to-delete");
    let field = Field::Primitive(Primitive::Integer(123));
    db.set(key.clone(), field).await.expect("set");
    db.flush().await.expect("flush");

    // delete
    db.delete(key.clone()).await.expect("delete");
    db.flush().await.expect("flush after delete");

    let res = db.get(&key).await;
    assert!(res.is_err(), "deleted key should not be found");

    drop(db);
    let _ = fs::remove_file(format!("{}.db", name)).await;
    let _ = fs::remove_file(format!("{}.dbidx", name)).await;
    let _ = fs::remove_file(format!("{}.wal", name)).await;
}
