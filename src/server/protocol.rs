use crate::dtypes::{Field, Key, PDBResult, bytes_from_field, field_from_bytes};
use crate::error::PeachDbError;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Read;

enum Command {
    Ping,
    Get,
    Set,
    Del,
    Keys,
}

impl TryFrom<u8> for Command {
    type Error = PeachDbError;
    fn try_from(b: u8) -> PDBResult<Self> {
        match b {
            0x01 => Ok(Command::Ping),
            0x02 => Ok(Command::Get),
            0x03 => Ok(Command::Set),
            0x04 => Ok(Command::Del),
            0x05 => Ok(Command::Keys),
            _ => Err(PeachDbError::UnknownCommand { byte: b }),
        }
    }
}

impl From<Command> for u8 {
    fn from(cmd: Command) -> u8 {
        match cmd {
            Command::Ping => 0x01,
            Command::Get => 0x02,
            Command::Set => 0x03,
            Command::Del => 0x04,
            Command::Keys => 0x05,
        }
    }
}

#[derive(Debug)]
pub enum Request {
    Ping,
    Get { key: Key },
    Set { key: Key, field: Field },
    Del { key: Key },
    Keys,
}

pub enum Status {
    Ok,
    NotFound,
    TypeError,
    ServerError,
    InvalidRequest,
    Keys,
}

impl TryFrom<u8> for Status {
    type Error = PeachDbError;
    fn try_from(b: u8) -> PDBResult<Self> {
        match b {
            0x00 => Ok(Status::Ok),
            0x01 => Ok(Status::NotFound),
            0x02 => Ok(Status::TypeError),
            0x03 => Ok(Status::ServerError),
            0x04 => Ok(Status::InvalidRequest),
            0x05 => Ok(Status::Keys),
            _ => Err(PeachDbError::UnknownCommand { byte: b }),
        }
    }
}

impl From<Status> for u8 {
    fn from(s: Status) -> u8 {
        match s {
            Status::Ok => 0x00,
            Status::NotFound => 0x01,
            Status::TypeError => 0x02,
            Status::ServerError => 0x03,
            Status::InvalidRequest => 0x04,
            Status::Keys => 0x05,
        }
    }
}

#[derive(Debug)]
pub enum Response {
    Ok(Option<Field>),
    NotFound,
    TypeError,
    ServerError(String),
    InvalidRequest,
    Keys(Vec<Key>),
}

pub struct ProtocolEncoder {
    buffer: Vec<u8>,
}

impl ProtocolEncoder {
    pub fn new() -> Self {
        ProtocolEncoder { buffer: Vec::new() }
    }

    fn write(&mut self, bytes: &[u8]) -> &mut Self {
        self.buffer.extend_from_slice(bytes);
        self
    }

    fn encode_u8(&mut self, val: u8) -> &mut Self {
        self.buffer.push(val);
        self
    }

    fn encode_u16_be(&mut self, val: u16) -> &mut Self {
        self.write(&val.to_be_bytes())
    }

    fn encode_u32_be(&mut self, val: u32) -> &mut Self {
        self.write(&val.to_be_bytes())
    }

    fn encode_key(&mut self, key: &Key) -> &mut Self {
        self.encode_u16_be(key.len() as u16)
            .write(key.as_bytes())
    }

    fn encode_field(&mut self, field: &Field) -> &mut Self {
        let bytes = bytes_from_field(field);
        self.encode_u32_be(bytes.len() as u32).write(&bytes)
    }

    pub fn encode_request(&mut self, req: &Request) -> &mut Self {
        self.encode_u8(0x01); // version
        match req {
            Request::Ping => {
                self.encode_u8(u8::from(Command::Ping));
            }
            Request::Keys => {
                self.encode_u8(u8::from(Command::Keys));
            }
            Request::Get { key } => {
                self.encode_u8(u8::from(Command::Get)).encode_key(key);
            }
            Request::Del { key } => {
                self.encode_u8(u8::from(Command::Del)).encode_key(key);
            }
            Request::Set { key, field } => {
                self.encode_u8(u8::from(Command::Set))
                    .encode_key(key)
                    .encode_field(field);
            }
        }
        self
    }

    pub fn encode_response(&mut self, res: &Response) -> &mut Self {
        match res {
            Response::Ok(maybe_field) => {
                self.encode_u8(u8::from(Status::Ok));
                match maybe_field {
                    None => {
                        self.encode_u32_be(0);
                    }
                    Some(field) => {
                        let bytes = bytes_from_field(field);
                        self.encode_u32_be(bytes.len() as u32).write(&bytes);
                    }
                }
            }
            Response::NotFound => {
                self.encode_u8(u8::from(Status::NotFound))
                    .encode_u32_be(0);
            }
            Response::TypeError => {
                self.encode_u8(u8::from(Status::TypeError))
                    .encode_u32_be(0);
            }
            Response::InvalidRequest => {
                self.encode_u8(u8::from(Status::InvalidRequest))
                    .encode_u32_be(0);
            }
            Response::ServerError(msg) => {
                let msg_bytes = msg.as_bytes();
                self.encode_u8(u8::from(Status::ServerError))
                    .encode_u32_be(msg_bytes.len() as u32)
                    .write(msg_bytes);
            }
            Response::Keys(keys) => {
                let mut payload = Vec::new();
                for key in keys {
                    payload.extend_from_slice(&(key.len() as u16).to_be_bytes());
                    payload.extend_from_slice(key.as_bytes());
                }
                self.encode_u8(u8::from(Status::Keys))
                    .encode_u32_be(payload.len() as u32)
                    .write(&payload);
            }
        }
        self
    }

    pub fn finish(&self) -> &[u8] {
        &self.buffer
    }

    pub fn frame(&self) -> Vec<u8> {
        let msg_len = self.buffer.len() as u32;
        let mut framed = Vec::with_capacity(4 + self.buffer.len());
        framed.extend_from_slice(&msg_len.to_be_bytes());
        framed.extend_from_slice(&self.buffer);
        framed
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
    }
}

pub struct ProtocolDecoder<'a> {
    cursor: std::io::Cursor<&'a [u8]>,
}

impl<'a> ProtocolDecoder<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        ProtocolDecoder {
            cursor: std::io::Cursor::new(bytes),
        }
    }

    fn read_u8(&mut self) -> PDBResult<u8> {
        Ok(ReadBytesExt::read_u8(&mut self.cursor)?)
    }

    fn read_u16_be(&mut self) -> PDBResult<u16> {
        Ok(ReadBytesExt::read_u16::<BigEndian>(&mut self.cursor)?)
    }

    fn read_u32_be(&mut self) -> PDBResult<u32> {
        Ok(ReadBytesExt::read_u32::<BigEndian>(&mut self.cursor)?)
    }

    fn read_key(&mut self) -> PDBResult<Key> {
        let len = self.read_u16_be()? as usize;
        let mut buf = vec![0u8; len];
        self.cursor.read_exact(&mut buf)?;
        Ok(Key::from(buf))
    }

    fn read_field(&mut self) -> PDBResult<Field> {
        let len = self.read_u32_be()? as usize;
        let mut buf = vec![0u8; len];
        self.cursor.read_exact(&mut buf)?;
        let (field, _) = field_from_bytes(&buf)?;
        Ok(field)
    }

    pub fn decode_request(&mut self) -> PDBResult<Request> {
        let version = self.read_u8()?;
        if version != 0x01 {
            return Err(PeachDbError::InvalidRequestFormat {
                reason: "invalid version".into(),
            });
        }
        let cmd = Command::try_from(self.read_u8()?)?;
        match cmd {
            Command::Ping => Ok(Request::Ping),
            Command::Keys => Ok(Request::Keys),
            Command::Get => Ok(Request::Get { key: self.read_key()? }),
            Command::Del => Ok(Request::Del { key: self.read_key()? }),
            Command::Set => {
                let key = self.read_key()?;
                let field = self.read_field()?;
                Ok(Request::Set { key, field })
            }
        }
    }

    pub fn decode_response(&mut self) -> PDBResult<Response> {
        let status = Status::try_from(self.read_u8()?)?;
        let payload_len = self.read_u32_be()? as usize;
        let mut payload = vec![0u8; payload_len];
        self.cursor.read_exact(&mut payload)?;

        match status {
            Status::Ok => {
                if payload.is_empty() {
                    Ok(Response::Ok(None))
                } else {
                    let (field, _) = field_from_bytes(&payload)?;
                    Ok(Response::Ok(Some(field)))
                }
            }
            Status::NotFound => Ok(Response::NotFound),
            Status::TypeError => Ok(Response::TypeError),
            Status::InvalidRequest => Ok(Response::InvalidRequest),
            Status::ServerError => {
                let msg = String::from_utf8(payload).map_err(|e| {
                    PeachDbError::InvalidRequestFormat {
                        reason: format!("invalid utf8 in error message: {}", e),
                    }
                })?;
                Ok(Response::ServerError(msg))
            }
            Status::Keys => {
                let mut keys = Vec::new();
                let mut cursor = std::io::Cursor::new(payload);
                while cursor.position() < cursor.get_ref().len() as u64 {
                    let key_len = ReadBytesExt::read_u16::<BigEndian>(&mut cursor)? as usize;
                    let mut key_buf = vec![0u8; key_len];
                    std::io::Read::read_exact(&mut cursor, &mut key_buf)?;
                    keys.push(Key::from(key_buf));
                }
                Ok(Response::Keys(keys))
            }
        }
    }
}
