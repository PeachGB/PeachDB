use std::sync::Arc;

use crate::error::PeachDbError;
pub const DTYPE_INFO_BYTE_SIZE: usize = 1;
pub const FIELD_LEN_BYTE_SIZE: usize = 4;
pub const KEY_LEN_BYTE_SIZE: usize = 2;

pub type PDBResult<T> = Result<T, PeachDbError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dtype {
    Raw,
    Integer,
    Float,
    String,
    Boolean,
    Array(u8),
}
impl Dtype {
    pub fn as_byte(&self) -> u8 {
        match self {
            Dtype::Raw => 0x0,
            Dtype::Integer => 0x1,
            Dtype::Float => 0x2,
            Dtype::String => 0x3,
            Dtype::Boolean => 0x4,
            Dtype::Array(of) => 0x10 | of,
        }
    }
}
impl TryFrom<u8> for Dtype {
    type Error = PeachDbError;

    fn try_from(b: u8) -> Result<Dtype, Self::Error> {
        match b {
            0x00 => Ok(Dtype::Raw),
            0x01 => Ok(Dtype::Integer),
            0x02 => Ok(Dtype::Float),
            0x03 => Ok(Dtype::String),
            0x04 => Ok(Dtype::Boolean),
            0x10..0x1F => Ok(Dtype::Array(b & 0x0F)),
            b => Err(PeachDbError::UnknownDtype { byte: b }),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Primitive {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Key(Vec<u8>);

impl AsRef<[u8]> for Key {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
impl From<Vec<u8>> for Key {
    fn from(key: Vec<u8>) -> Self {
        Key(key)
    }
}
impl From<&[u8]> for Key {
    fn from(key: &[u8]) -> Self {
        Key(Vec::from(key))
    }
}
impl From<&str> for Key {
    fn from(key: &str) -> Self {
        Key(Vec::from(key))
    }
}
impl Key {
    pub fn empty() -> Self {
        Key(Vec::new())
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
    //returns the number of bytes in the vector that contains the key
    pub fn len(&self) -> usize {
        self.0.len()
    }
    ///returns the value of the elements in the vector plus the lenght of the bytes that contains the key size information. as the vector a Vec<u8> this returns the number of bytes
    pub fn size(&self) -> usize {
        KEY_LEN_BYTE_SIZE + self.0.len()
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match std::str::from_utf8(&self.0) {
            Ok(s) => write!(f, "{}", s),
            Err(_) => write!(f, "{:?}", self.0),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Field {
    Primitive(Primitive),
    Raw(Arc<[u8]>),
    Array(Dtype, Arc<[Primitive]>),
}
impl Field {
    pub fn zero() -> Self {
        Field::Primitive(Primitive::Integer(0))
    }

    pub fn len(&self) -> usize {
        match self {
            Field::Primitive(_p) => 1,
            Field::Raw(bytes) => bytes.len(),
            Field::Array(_dtype, a) => a.len(),
        }
    }
    pub fn dtype(&self) -> Dtype {
        match self {
            Field::Primitive(p) => match p {
                Primitive::Integer(_) => Dtype::Integer,
                Primitive::Float(_) => Dtype::Float,
                Primitive::String(_) => Dtype::String,
                Primitive::Boolean(_) => Dtype::Boolean,
            },
            Field::Array(dtype, _a) => Dtype::Array(dtype.as_byte()),
            Field::Raw(_) => Dtype::Raw,
        }
    }
}

pub fn bytes_from_primitive(p: &Primitive) -> Vec<u8> {
    match p {
        Primitive::Integer(n) => n.to_le_bytes().to_vec(),
        Primitive::Float(f) => f.to_le_bytes().to_vec(),
        Primitive::String(s) => {
            let mut b = Vec::with_capacity(FIELD_LEN_BYTE_SIZE + s.len());
            b.extend_from_slice(&(s.len() as u32).to_le_bytes());
            b.extend_from_slice(s.as_bytes());
            b
        }
        Primitive::Boolean(bool) => vec![*bool as u8],
    }
}

pub fn primitive_from_bytes(dt: &Dtype, buf: &[u8]) -> PDBResult<(Primitive, usize)> {
    match dt {
        Dtype::Integer => {
            if buf.len() < 8 {
                return Err(PeachDbError::BufferTooShort);
            }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&buf[0..8]);
            Ok((Primitive::Integer(i64::from_le_bytes(arr)), 8))
        }
        Dtype::Float => {
            if buf.len() < 8 {
                return Err(PeachDbError::BufferTooShort);
            }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&buf[0..8]);
            Ok((Primitive::Float(f64::from_le_bytes(arr)), 8))
        }
        Dtype::Boolean => Ok((Primitive::Boolean(buf[0] != 0), 1)),
        Dtype::String => {
            if buf.len() < 4 {
                return Err(PeachDbError::BufferTooShort);
            }
            let len = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
            let s = String::from_utf8(buf[4..4 + len].to_vec()).map_err(|e| {
                PeachDbError::CorruptedRecord {
                    reason: format!("invalid utf8: {}", e),
                }
            })?;
            Ok((Primitive::String(s), FIELD_LEN_BYTE_SIZE + len))
        }
        Dtype::Raw => Err(PeachDbError::UnknownDtype { byte: dt.as_byte() }),
        Dtype::Array(_) => Err(PeachDbError::UnknownDtype { byte: dt.as_byte() }),
    }
}

pub fn bytes_from_field(f: &Field) -> Vec<u8> {
    match f {
        Field::Primitive(p) => {
            let primitive = bytes_from_primitive(p);
            let mut out = Vec::with_capacity(1 + primitive.len());
            out.push(f.dtype().as_byte());
            out.extend_from_slice(&primitive);
            out
        }
        Field::Raw(bytes) => {
            let mut out = Vec::with_capacity(1 + bytes.len());
            out.push(f.dtype().as_byte());
            out.extend_from_slice(bytes);
            out
        }
        Field::Array(dt, array) => {
            let mut array_as_bytes = Vec::new();
            let count = array.len() as u32;
            array_as_bytes.push(Dtype::Array(dt.as_byte()).as_byte());
            array_as_bytes.extend_from_slice(&count.to_le_bytes());
            for item in array.iter() {
                array_as_bytes.extend_from_slice(&bytes_from_primitive(item));
            }
            array_as_bytes
        }
    }
}

pub fn field_from_bytes(buf: &[u8]) -> PDBResult<(Field, usize)> {
    if buf.is_empty() {
        return Err(PeachDbError::BufferTooShort);
    }

    let dtype = Dtype::try_from(buf[0])?;
    let payload = &buf[1..];

    match dtype {
        Dtype::Integer | Dtype::Float | Dtype::String | Dtype::Boolean => {
            let (p, c) = primitive_from_bytes(&dtype, payload)?;
            Ok((Field::Primitive(p), 1 + c))
        }
        Dtype::Raw => {
            let arc: Arc<[u8]> = payload.to_vec().into_boxed_slice().into();
            Ok((Field::Raw(arc), 1 + payload.len()))
        }
        Dtype::Array(inner) => {
            if payload.len() < 4 {
                return Err(PeachDbError::BufferTooShort);
            }
            let count = u32::from_le_bytes(payload[0..4].try_into().unwrap()) as usize;
            let mut offset = 4usize;
            let mut v: Vec<Primitive> = Vec::with_capacity(count);
            let inner_dt = Dtype::try_from(inner)?;
            for _ in 0..count {
                let (p, consumed) = primitive_from_bytes(&inner_dt, &payload[offset..])?;
                offset += consumed;
                v.push(p);
            }
            let arc: Arc<[Primitive]> = v.into_boxed_slice().into();
            Ok((Field::Array(inner_dt, arc), 1 + offset))
        }
    }
}
