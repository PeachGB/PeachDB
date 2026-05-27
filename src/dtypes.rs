use std::sync::Arc;

use crate::error::PeachDbError;
pub const DTYPE_INFO_BYTE_SIZE: usize = 1;
pub const FIELD_LEN_BYTE_SIZE: usize = 4;
pub const KEY_LEN_BYTE_SIZE: usize = 2;

pub type PDBResult<T> = Result<T, PeachDbError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dtype {
    Integer,
    Float,
    String,
    Boolean,
    Array(u8),
}
impl Dtype {
    pub fn as_byte(&self) -> u8 {
        match self {
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
            0x01 => Ok(Dtype::Integer),
            0x02 => Ok(Dtype::Float),
            0x03 => Ok(Dtype::String),
            0x04 => Ok(Dtype::Boolean),
            0x10..0x1F => Ok(Dtype::Array(b & 0x0F)),
            b => Err(PeachDbError::UnknownDtype { byte: b }),
        }
    }
}

#[derive(Clone)]
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

#[derive(Clone)]
pub enum Field {
    Primitive(Primitive),
    Array(Dtype, Arc<[Primitive]>),
}
impl Field {
    pub fn zero() -> Self {
        Field::Primitive(Primitive::Integer(0))
    }

    pub fn len(&self) -> usize {
        match self {
            Field::Primitive(_p) => 1,
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
        }
    }
}
