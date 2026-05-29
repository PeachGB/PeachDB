use crate::dtypes::{Field, Key, Primitive};
use crate::server::protocol::{ProtocolDecoder, ProtocolEncoder, Request, Response};
use std::sync::Arc;

// ── helpers ─────────────────────────────────────────────────────────────────

fn roundtrip_request(req: &Request) -> Request {
    let mut enc = ProtocolEncoder::new();
    enc.encode_request(req);
    ProtocolDecoder::new(enc.finish())
        .decode_request()
        .expect("decode_request failed")
}

fn roundtrip_response(res: &Response) -> Response {
    let mut enc = ProtocolEncoder::new();
    enc.encode_response(res);
    ProtocolDecoder::new(enc.finish())
        .decode_response()
        .expect("decode_response failed")
}

// ── request roundtrips ───────────────────────────────────────────────────────

#[test]
fn request_ping_roundtrip() {
    matches!(roundtrip_request(&Request::Ping), Request::Ping);
}

#[test]
fn request_keys_roundtrip() {
    matches!(roundtrip_request(&Request::Keys), Request::Keys);
}

#[test]
fn request_get_roundtrip() {
    let key = Key::from("some-key");
    let req = roundtrip_request(&Request::Get { key: key.clone() });
    match req {
        Request::Get { key: k } => assert_eq!(k.as_bytes(), key.as_bytes()),
        _ => panic!("expected Get"),
    }
}

#[test]
fn request_del_roundtrip() {
    let key = Key::from("del-key");
    let req = roundtrip_request(&Request::Del { key: key.clone() });
    match req {
        Request::Del { key: k } => assert_eq!(k.as_bytes(), key.as_bytes()),
        _ => panic!("expected Del"),
    }
}

#[test]
fn request_set_integer_roundtrip() {
    let key = Key::from("int-key");
    let field = Field::Primitive(Primitive::Integer(42));
    let req = roundtrip_request(&Request::Set { key: key.clone(), field });
    match req {
        Request::Set { key: k, field: Field::Primitive(Primitive::Integer(n)) } => {
            assert_eq!(k.as_bytes(), key.as_bytes());
            assert_eq!(n, 42);
        }
        _ => panic!("expected Set with Integer"),
    }
}

#[test]
fn request_set_string_roundtrip() {
    let key = Key::from("str-key");
    let field = Field::Primitive(Primitive::String("hello".to_string()));
    let req = roundtrip_request(&Request::Set { key: key.clone(), field });
    match req {
        Request::Set { key: k, field: Field::Primitive(Primitive::String(s)) } => {
            assert_eq!(k.as_bytes(), key.as_bytes());
            assert_eq!(s, "hello");
        }
        _ => panic!("expected Set with String"),
    }
}

#[test]
fn request_set_boolean_roundtrip() {
    let key = Key::from("bool-key");
    let field = Field::Primitive(Primitive::Boolean(true));
    let req = roundtrip_request(&Request::Set { key: key.clone(), field });
    match req {
        Request::Set { key: k, field: Field::Primitive(Primitive::Boolean(b)) } => {
            assert_eq!(k.as_bytes(), key.as_bytes());
            assert!(b);
        }
        _ => panic!("expected Set with Boolean"),
    }
}

#[test]
fn request_set_raw_roundtrip() {
    let key = Key::from("raw-key");
    let bytes: Arc<[u8]> = vec![0xDE, 0xAD, 0xBE, 0xEF].into_boxed_slice().into();
    let field = Field::Raw(bytes.clone());
    let req = roundtrip_request(&Request::Set { key: key.clone(), field });
    match req {
        Request::Set { key: k, field: Field::Raw(b) } => {
            assert_eq!(k.as_bytes(), key.as_bytes());
            assert_eq!(&*b, &*bytes);
        }
        _ => panic!("expected Set with Raw"),
    }
}

// ── response roundtrips ──────────────────────────────────────────────────────

#[test]
fn response_ok_none_roundtrip() {
    match roundtrip_response(&Response::Ok(None)) {
        Response::Ok(None) => {}
        _ => panic!("expected Ok(None)"),
    }
}

#[test]
fn response_ok_some_roundtrip() {
    let field = Field::Primitive(Primitive::Float(3.14));
    match roundtrip_response(&Response::Ok(Some(field))) {
        Response::Ok(Some(Field::Primitive(Primitive::Float(f)))) => {
            assert!((f - 3.14).abs() < f64::EPSILON);
        }
        _ => panic!("expected Ok(Some(Float))"),
    }
}

#[test]
fn response_not_found_roundtrip() {
    matches!(roundtrip_response(&Response::NotFound), Response::NotFound);
}

#[test]
fn response_type_error_roundtrip() {
    matches!(roundtrip_response(&Response::TypeError), Response::TypeError);
}

#[test]
fn response_invalid_request_roundtrip() {
    matches!(
        roundtrip_response(&Response::InvalidRequest),
        Response::InvalidRequest
    );
}

#[test]
fn response_server_error_roundtrip() {
    let msg = "something exploded".to_string();
    match roundtrip_response(&Response::ServerError(msg.clone())) {
        Response::ServerError(m) => assert_eq!(m, msg),
        _ => panic!("expected ServerError"),
    }
}

#[test]
fn response_keys_roundtrip() {
    let keys = vec![Key::from("alpha"), Key::from("beta"), Key::from("gamma")];
    match roundtrip_response(&Response::Keys(keys.clone())) {
        Response::Keys(ks) => {
            assert_eq!(ks.len(), keys.len());
            for (a, b) in ks.iter().zip(keys.iter()) {
                assert_eq!(a.as_bytes(), b.as_bytes());
            }
        }
        _ => panic!("expected Keys"),
    }
}

#[test]
fn response_keys_empty_roundtrip() {
    match roundtrip_response(&Response::Keys(vec![])) {
        Response::Keys(ks) => assert!(ks.is_empty()),
        _ => panic!("expected Keys"),
    }
}

// ── error cases ──────────────────────────────────────────────────────────────

#[test]
fn decode_request_wrong_version() {
    let bytes = [0x02, 0x01]; // version=2, cmd=Ping
    let err = ProtocolDecoder::new(&bytes)
        .decode_request()
        .unwrap_err();
    assert!(err.to_string().contains("invalid version"));
}

#[test]
fn decode_request_unknown_command() {
    let bytes = [0x01, 0xFF]; // version=1, cmd=unknown
    let err = ProtocolDecoder::new(&bytes)
        .decode_request()
        .unwrap_err();
    assert!(err.to_string().contains("0xff"));
}

#[test]
fn decode_response_unknown_status() {
    let bytes = [0xFF, 0x00, 0x00, 0x00, 0x00]; // status=unknown, payload_len=0
    let err = ProtocolDecoder::new(&bytes)
        .decode_response()
        .unwrap_err();
    assert!(err.to_string().contains("0xff"));
}

#[test]
fn encoder_reset_clears_buffer() {
    let mut enc = ProtocolEncoder::new();
    enc.encode_request(&Request::Ping);
    assert!(!enc.finish().is_empty());
    enc.reset();
    assert!(enc.finish().is_empty());
}
