use super::helpers::{DbCleanup, unique_db_name};
use crate::db::Database;
use crate::dtypes::{Field, Key, Primitive};
use crate::server::protocol::{ProtocolDecoder, ProtocolEncoder, Request, Response};
use crate::server::Server;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ── TCP helpers ──────────────────────────────────────────────────────────────

async fn send_request(stream: &mut TcpStream, req: &Request) {
    let mut enc = ProtocolEncoder::new();
    enc.encode_request(req);
    stream.write_all(&enc.frame()).await.unwrap();
}

async fn read_response(stream: &mut TcpStream) -> Response {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.unwrap();
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await.unwrap();
    ProtocolDecoder::new(&buf).decode_response().unwrap()
}

async fn open_db(name: &str) -> Database {
    Database::open(name.to_string()).await.expect("open db")
}

/// Creates a connected (client, server-side) TcpStream pair via a local listener.
async fn tcp_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let client = TcpStream::connect(addr).await.unwrap();
    let (server_side, _) = listener.accept().await.unwrap();
    (client, server_side)
}

// ── handle_request unit tests ────────────────────────────────────────────────

#[tokio::test]
async fn handle_request_ping() {
    let name = unique_db_name("peachdb_srv_test");
    let _cleanup = DbCleanup { name: name.clone() };
    let db = open_db(&name).await;

    match Server::handle_request(&db, Request::Ping).await {
        Response::Ok(None) => {}
        r => panic!("expected Ok(None), got {:?}", r),
    }
}

#[tokio::test]
async fn handle_request_get_missing_key() {
    let name = unique_db_name("peachdb_srv_test");
    let _cleanup = DbCleanup { name: name.clone() };
    let db = open_db(&name).await;

    match Server::handle_request(&db, Request::Get { key: Key::from("no-such-key") }).await {
        Response::NotFound => {}
        r => panic!("expected NotFound, got {:?}", r),
    }
}

#[tokio::test]
async fn handle_request_set_then_get() {
    let name = unique_db_name("peachdb_srv_test");
    let _cleanup = DbCleanup { name: name.clone() };
    let db = open_db(&name).await;

    let key = Key::from("k1");
    let field = Field::Primitive(Primitive::Integer(99));

    match Server::handle_request(&db, Request::Set { key: key.clone(), field: field.clone() }).await {
        Response::Ok(None) => {}
        r => panic!("expected Ok(None) from set, got {:?}", r),
    }

    match Server::handle_request(&db, Request::Get { key: key.clone() }).await {
        Response::Ok(Some(Field::Primitive(Primitive::Integer(n)))) => assert_eq!(n, 99),
        r => panic!("expected Ok(Some(Integer(99))), got {:?}", r),
    }
}

#[tokio::test]
async fn handle_request_del_existing() {
    let name = unique_db_name("peachdb_srv_test");
    let _cleanup = DbCleanup { name: name.clone() };
    let db = open_db(&name).await;

    let key = Key::from("del-me");
    db.set(key.clone(), Field::Primitive(Primitive::Boolean(true))).await.unwrap();

    match Server::handle_request(&db, Request::Del { key: key.clone() }).await {
        Response::Ok(None) => {}
        r => panic!("expected Ok(None) from del, got {:?}", r),
    }

    match Server::handle_request(&db, Request::Get { key }).await {
        Response::NotFound => {}
        r => panic!("expected NotFound after delete, got {:?}", r),
    }
}

#[tokio::test]
async fn handle_request_del_missing_key_is_ok() {
    // delete is idempotent: deleting a non-existent key returns Ok(None)
    let name = unique_db_name("peachdb_srv_test");
    let _cleanup = DbCleanup { name: name.clone() };
    let db = open_db(&name).await;

    match Server::handle_request(&db, Request::Del { key: Key::from("ghost") }).await {
        Response::Ok(None) => {}
        r => panic!("expected Ok(None), got {:?}", r),
    }
}

#[tokio::test]
async fn handle_request_keys_empty() {
    let name = unique_db_name("peachdb_srv_test");
    let _cleanup = DbCleanup { name: name.clone() };
    let db = open_db(&name).await;

    match Server::handle_request(&db, Request::Keys).await {
        Response::Keys(ks) => assert!(ks.is_empty()),
        r => panic!("expected Keys([]), got {:?}", r),
    }
}

#[tokio::test]
async fn handle_request_keys_after_set() {
    let name = unique_db_name("peachdb_srv_test");
    let _cleanup = DbCleanup { name: name.clone() };
    let db = open_db(&name).await;

    let key = Key::from("alpha");
    db.set(key.clone(), Field::Primitive(Primitive::Integer(1))).await.unwrap();

    match Server::handle_request(&db, Request::Keys).await {
        Response::Keys(ks) => {
            assert_eq!(ks.len(), 1);
            assert_eq!(ks[0].as_bytes(), key.as_bytes());
        }
        r => panic!("expected Keys([alpha]), got {:?}", r),
    }
}

// ── handle_connection integration tests ─────────────────────────────────────

#[tokio::test]
async fn connection_ping_roundtrip() {
    let name = unique_db_name("peachdb_srv_test");
    let _cleanup = DbCleanup { name: name.clone() };
    let db = Arc::new(open_db(&name).await);

    let (mut client, server_stream) = tcp_pair().await;
    tokio::spawn(Server::handle_connection(Arc::clone(&db), server_stream));

    send_request(&mut client, &Request::Ping).await;
    match read_response(&mut client).await {
        Response::Ok(None) => {}
        r => panic!("expected Ok(None), got {:?}", r),
    }
}

#[tokio::test]
async fn connection_set_get_roundtrip() {
    let name = unique_db_name("peachdb_srv_test");
    let _cleanup = DbCleanup { name: name.clone() };
    let db = Arc::new(open_db(&name).await);

    let (mut client, server_stream) = tcp_pair().await;
    tokio::spawn(Server::handle_connection(Arc::clone(&db), server_stream));

    let key = Key::from("tcp-key");
    let field = Field::Primitive(Primitive::String("tcp-val".to_string()));

    send_request(&mut client, &Request::Set { key: key.clone(), field }).await;
    match read_response(&mut client).await {
        Response::Ok(None) => {}
        r => panic!("expected Ok(None) from set, got {:?}", r),
    }

    send_request(&mut client, &Request::Get { key }).await;
    match read_response(&mut client).await {
        Response::Ok(Some(Field::Primitive(Primitive::String(s)))) => {
            assert_eq!(s, "tcp-val");
        }
        r => panic!("expected Ok(Some(String)), got {:?}", r),
    }
}

#[tokio::test]
async fn connection_invalid_request_continues() {
    let name = unique_db_name("peachdb_srv_test");
    let _cleanup = DbCleanup { name: name.clone() };
    let db = Arc::new(open_db(&name).await);

    let (mut client, server_stream) = tcp_pair().await;
    tokio::spawn(Server::handle_connection(Arc::clone(&db), server_stream));

    // Send an invalid frame: version byte = 0xFF
    let bad_payload = vec![0xFF, 0x01];
    let mut bad_frame = (bad_payload.len() as u32).to_be_bytes().to_vec();
    bad_frame.extend(bad_payload);
    client.write_all(&bad_frame).await.unwrap();

    match read_response(&mut client).await {
        Response::InvalidRequest => {}
        r => panic!("expected InvalidRequest, got {:?}", r),
    }

    // Connection should still be alive — send a valid Ping
    send_request(&mut client, &Request::Ping).await;
    match read_response(&mut client).await {
        Response::Ok(None) => {}
        r => panic!("expected Ok(None) after recovery, got {:?}", r),
    }
}

#[tokio::test]
async fn connection_keys_roundtrip() {
    let name = unique_db_name("peachdb_srv_test");
    let _cleanup = DbCleanup { name: name.clone() };
    let db = Arc::new(open_db(&name).await);

    let (mut client, server_stream) = tcp_pair().await;
    tokio::spawn(Server::handle_connection(Arc::clone(&db), server_stream));

    let key = Key::from("key-a");
    send_request(&mut client, &Request::Set {
        key: key.clone(),
        field: Field::Primitive(Primitive::Integer(42)),
    }).await;
    read_response(&mut client).await; // consume Ok(None)

    send_request(&mut client, &Request::Keys).await;
    match read_response(&mut client).await {
        Response::Keys(ks) => {
            assert_eq!(ks.len(), 1);
            assert_eq!(ks[0].as_bytes(), key.as_bytes());
        }
        r => panic!("expected Keys, got {:?}", r),
    }
}
