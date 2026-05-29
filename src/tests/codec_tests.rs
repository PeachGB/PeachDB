use crate::db::{encode, decode};
use crate::dtypes::{Dtype, Primitive, Field, Key, bytes_from_primitive, primitive_from_bytes, bytes_from_field, field_from_bytes};
use std::sync::Arc;

#[test]
fn primitive_integer_roundtrip() {
    let p = Primitive::Integer(42);
    let b = bytes_from_primitive(&p);
    let (p2, _c) = primitive_from_bytes(&Dtype::Integer, &b).expect("primitive decode");
    match p2 {
        Primitive::Integer(n) => assert_eq!(n, 42),
        _ => panic!("unexpected primitive variant"),
    }
}

#[test]
fn primitive_string_roundtrip_in_field() {
    let f = Field::Primitive(Primitive::String("hello".to_string()));
    let fb = bytes_from_field(&f);
    let (f2, _c) = field_from_bytes(&fb).expect("field decode");
    match f2 {
        Field::Primitive(Primitive::String(s)) => assert_eq!(s, "hello"),
        _ => panic!("unexpected field variant"),
    }
}

#[test]
fn encode_decode_record_roundtrip() {
    let key = Key::from("mykey");
    let field = Field::Primitive(Primitive::Boolean(true));
    let rec = encode(&key, &field);
    let (k2, f2) = decode(&rec).expect("record decode");
    assert_eq!(k2.as_bytes(), key.as_bytes());
    match f2 {
        Field::Primitive(Primitive::Boolean(b)) => assert!(b),
        _ => panic!("unexpected field variant"),
    }
}

#[test]
fn array_field_roundtrip() {
    let v = vec![Primitive::Integer(1), Primitive::Integer(2)];
    let arc: Arc<[Primitive]> = Arc::from(v.into_boxed_slice());
    let f = Field::Array(Dtype::Integer, arc);
    let fb = bytes_from_field(&f);
    let (f2, _c) = field_from_bytes(&fb).expect("array decode");
    match f2 {
        Field::Array(dt, arr) => {
            assert_eq!(dt, Dtype::Integer);
            assert_eq!(arr.len(), 2);
            match &arr[0] {
                Primitive::Integer(n) => assert_eq!(*n, 1),
                _ => panic!("unexpected element"),
            }
        }
        _ => panic!("unexpected field variant"),
    }
}
