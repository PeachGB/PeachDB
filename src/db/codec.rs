use crate::{
    db::{DbMeta, IndexEntry, wal::WalEntry},
    dtypes::{
        Field, FIELD_LEN_BYTE_SIZE, KEY_LEN_BYTE_SIZE, Key, PDBResult,
        bytes_from_field, field_from_bytes,
    },
    error::PeachDbError,
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
};

use crc::{CRC_32_ISO_HDLC, Crc};

const RECORD_MAGIC_NUMBER_BYTE_SIZE: usize = 4;
const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);
const RECORD_MAGIC_NUMBER: [u8; 4] = [b'R', b'E', b'C', 0x01];
const CRC_OFFSET: usize = RECORD_MAGIC_NUMBER_BYTE_SIZE + KEY_LEN_BYTE_SIZE + FIELD_LEN_BYTE_SIZE;

pub struct DBEncoder {
    buffer: Vec<u8>,
    field_start_position: usize,
}
impl DBEncoder {
    pub fn new() -> Self {
        DBEncoder {
            buffer: Vec::new(),
            field_start_position: 0,
        }
    }
    pub fn with_capacity(capacity: usize) -> Self {
        let buffer = Vec::with_capacity(capacity);
        DBEncoder {
            buffer,
            field_start_position: 0,
        }
    }
    fn write(&mut self, buffer: &[u8]) -> &mut Self {
        self.buffer.extend_from_slice(buffer);
        self
    }
    fn encode_magic_number(&mut self) -> &mut Self {
        self.field_start_position = self.buffer.len();
        self.write(&RECORD_MAGIC_NUMBER)
    }

    fn encode_key_len_u16(&mut self, len: usize) -> &mut Self {
        let key_len: [u8; 2] = (len as u16).to_le_bytes();
        self.write(&key_len)
    }

    fn encode_field_len_u32(&mut self, len: usize) -> &mut Self {
        let field_len: [u8; 4] = (len as u32).to_le_bytes();
        self.write(&field_len)
    }

    fn encode_key(&mut self, key: &Key) -> &mut Self {
        self.write(key.as_bytes())
    }
    fn encode_crc_placeholder(&mut self) -> &mut Self {
        self.write(&[0u8; 4])
    }
    //this method assumes that the field payload has already been appended
    fn encode_crc(&mut self) -> &mut Self {
        let checksum = CRC.checksum(&self.buffer[self.field_start_position + 4..]);
        let offset = self.field_start_position + CRC_OFFSET;
        self.buffer[offset..offset + 4usize].copy_from_slice(&checksum.to_le_bytes());
        self
    }

    /// High-level encode: wraps record header, crc and payload
    pub fn encode_db_key_field_pair(&mut self, key: &Key, field: &Field) -> &mut Self {
        let payload = bytes_from_field(field);
        self.encode_magic_number()
            .encode_key_len_u16(key.len())
            .encode_field_len_u32(payload.len())
            .encode_crc_placeholder()
            .encode_key(key)
            .write(&payload)
            .encode_crc()
    }

    fn encode_db_header_key(&mut self) -> &mut Self {
        self.buffer.extend_from_slice(&DB_HEADER_KEY);
        self
    }
    fn encode_db_header_magic(&mut self) -> &mut Self {
        self.buffer
            .extend_from_slice(&DB_HEADER_MAGIC.to_le_bytes());
        self
    }
    fn encode_db_version_key(&mut self) -> &mut Self {
        self.buffer.extend_from_slice(&DB_VERSION_KEY);
        self
    }
    fn encode_db_version(&mut self) -> &mut Self {
        self.buffer.push(DB_VERSION);
        self
    }
    fn encode_db_version_decimal(&mut self) -> &mut Self {
        self.buffer.push(DB_VERSION_DECIMAL);
        self
    }

    fn encode_db_rc_key(&mut self) -> &mut Self {
        self.buffer.extend_from_slice(&DB_RECORD_COUNT_KEY);
        self
    }
    fn encode_db_rc(&mut self, rc: u64) -> &mut Self {
        self.buffer.extend_from_slice(&rc.to_le_bytes());
        self
    }
    fn encode_db_name_key(&mut self) -> &mut Self {
        self.buffer.extend_from_slice(&DB_NAME_KEY);
        self
    }
    fn encode_db_name(&mut self, name: String) -> &mut Self {
        let mut name_bytes = [0u8; DB_NAME_LEN];
        let name_as_bytes = name.as_bytes();
        let copy_len = name_as_bytes.len().min(DB_NAME_LEN);
        name_bytes[..copy_len].copy_from_slice(&name_as_bytes[..copy_len]);
        self.buffer.extend_from_slice(&name_bytes);
        self
    }
    pub fn encode_db_header(&mut self, meta: &DbMeta) -> &mut Self {
        self.encode_db_header_key()
            .encode_db_header_magic()
            .encode_db_version_key()
            .encode_db_version()
            .encode_db_version_decimal()
            .encode_db_rc_key()
            .encode_db_rc(meta.record_count)
            .encode_db_name_key()
            .encode_db_name(meta.name.clone())
    }

    pub fn encode_db_file(&mut self, fields: &HashMap<Key, Field>, meta: DbMeta) -> &mut Self {
        self.encode_db_header(&meta);

        for (key, field) in fields.iter() {
            self.encode_db_key_field_pair(key, field);
        }
        self
    }
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.field_start_position = 0;
    }
    pub fn finish(&self) -> &[u8] {
        &self.buffer
    }
}

impl Default for DBEncoder {
    fn default() -> Self {
        Self::new()
    }
}

const CRC_BYTE_SIZE: usize = 4;

/// Convenience wrapper to encode a single key/field record into bytes
pub fn encode(key: &Key, field: &Field) -> Vec<u8> {
    let payload = bytes_from_field(field);
    let mut enc = DBEncoder::with_capacity(
        RECORD_MAGIC_NUMBER_BYTE_SIZE
            + KEY_LEN_BYTE_SIZE
            + FIELD_LEN_BYTE_SIZE
            + CRC_BYTE_SIZE
            + key.len()
            + payload.len(),
    );
    enc.encode_db_key_field_pair(key, field);
    enc.finish().to_vec()
}

/// Convenience wrapper to decode a single key/field record from bytes
pub fn decode(bytes: &[u8]) -> PDBResult<(Key, Field)> {
    let mut dec = DBDecoder::new(bytes);
    dec.decode_db_key_field_pair()?;
    // consume db and return the first entry
    let mut iter = dec.db.into_iter();
    if let Some((k, v)) = iter.next() {
        Ok((k, v))
    } else {
        Err(PeachDbError::InvalidFinishCall)
    }
}

const DB_HEADER_KEY: [u8; 8] = *b"Header--";
const DB_HEADER_MAGIC: u64 = 0x50656163684442;
const DB_VERSION_KEY: [u8; 8] = *b"Version-";
const DB_VERSION: u8 = 1;
const DB_VERSION_DECIMAL: u8 = 0;
const DB_RECORD_COUNT_KEY: [u8; 8] = *b"RecCount";
//record count:u64
const DB_NAME_KEY: [u8; 8] = *b"DB Name-";
const DB_NAME_LEN: usize = 64;

pub struct DBDecoder<'a> {
    cursor: Cursor<&'a [u8]>,
    db: HashMap<Key, Field>,
    meta: Option<DbMeta>,
    deleted: bool,
    last_key_len: u16,
    last_field_len: u32,
    last_crc: u32,
    last_key: Vec<u8>,
    last_field: Vec<u8>,
}
impl<'a> DBDecoder<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        DBDecoder {
            cursor: Cursor::new(bytes),
            db: HashMap::new(),
            meta: None,
            last_key_len: 0u16,
            last_field_len: 0u32,
            last_crc: 0u32,
            last_key: Vec::new(),
            last_field: Vec::new(),
            deleted: false,
        }
    }

    fn read_magic_number(&mut self) -> PDBResult<&mut Self> {
        let mut rec_magic_number = [0u8; 4];
        self.cursor.read_exact(&mut rec_magic_number)?;
        match rec_magic_number {
            [0, 0, 0, 0] => {
                self.deleted = true;
                Ok(self)
            }
            RECORD_MAGIC_NUMBER => {
                self.deleted = false;
                Ok(self)
            }
            _ => Err(PeachDbError::InvalidMagicBytes),
        }
    }
    fn read_key_len(&mut self) -> PDBResult<&mut Self> {
        let mut key_len_bytes = [0u8; 2];
        self.cursor.read_exact(&mut key_len_bytes)?;
        self.last_key_len = u16::from_le_bytes(key_len_bytes);
        Ok(self)
    }
    fn read_field_len(&mut self) -> PDBResult<&mut Self> {
        let mut field_len_bytes = [0u8; 4];
        self.cursor.read_exact(&mut field_len_bytes)?;
        self.last_field_len = u32::from_le_bytes(field_len_bytes);
        Ok(self)
    }
    fn read_crc(&mut self) -> PDBResult<&mut Self> {
        let mut crc_bytes = [0u8; 4];
        self.cursor.read_exact(&mut crc_bytes)?;

        self.last_crc = u32::from_le_bytes(crc_bytes);
        Ok(self)
    }
    fn read_key(&mut self) -> PDBResult<&mut Self> {
        let mut key_bytes = vec![0u8; self.last_key_len as usize];

        self.cursor.read_exact(&mut key_bytes)?;
        self.last_key = key_bytes;
        Ok(self)
    }
    fn read_field(&mut self) -> PDBResult<&mut Self> {
        let mut field_bytes = vec![0u8; self.last_field_len as usize];
        self.cursor.read_exact(&mut field_bytes)?;
        self.last_field = field_bytes;

        Ok(self)
    }
    fn check_crc(&mut self) -> PDBResult<&mut Self> {
        let header_for_crc_capacity =
            2 + 4 + 4 + self.last_key_len as usize + self.last_field_len as usize;
        let mut header_for_crc = Vec::with_capacity(header_for_crc_capacity);
        header_for_crc.extend_from_slice(&self.last_key_len.to_le_bytes());
        header_for_crc.extend_from_slice(&self.last_field_len.to_le_bytes());
        header_for_crc.extend_from_slice(&[0u8; 4]); // crc en cero
        header_for_crc.extend_from_slice(&self.last_key);
        header_for_crc.extend_from_slice(&self.last_field);

        let computed = CRC.checksum(&header_for_crc);
        if computed != self.last_crc {
            return Err(PeachDbError::CorruptedRecord {
                reason: String::from("Checksum Mismatch"),
            });
        }
        Ok(self)
    }
    /// High-level decode: validates magic and crc and returns Key and Field
    pub fn decode_db_key_field_pair(&mut self) -> PDBResult<&mut Self> {
        self.read_magic_number()?
            .read_key_len()?
            .read_field_len()?
            .read_crc()?
            .read_key()?
            .read_field()?;

        if !self.deleted {
            self.check_crc()?;
            let (field, _) = field_from_bytes(&self.last_field)?;
            self.db.insert(Key::from(self.last_key.clone()), field);
        }
        Ok(self)
    }

    fn is_metadata_key(key: &[u8; 8]) -> bool {
        key == &DB_HEADER_KEY
            || key == &DB_VERSION_KEY
            || key == &DB_RECORD_COUNT_KEY
            || key == &DB_NAME_KEY
    }
    fn read_metadata_key(&mut self) -> PDBResult<&mut Self> {
        let mut key_buf = [0u8; 8];
        self.cursor.read_exact(&mut key_buf)?;
        if Self::is_metadata_key(&key_buf) {
            Ok(self)
        } else {
            Err(PeachDbError::InvalidMagicBytes)
        }
    }
    fn decode_db_header(&mut self) -> PDBResult<&mut Self> {
        self.cursor.set_position(0);

        self.read_metadata_key()?;
        let _header = self.cursor.read_u64::<LittleEndian>()?;
        self.read_metadata_key()?;
        let version = self.cursor.read_u8()?;
        let _version_decimal = self.cursor.read_u8()?;
        self.read_metadata_key()?;
        let record_count = self.cursor.read_u64::<LittleEndian>()?;
        self.read_metadata_key()?;
        let mut db_name: [u8; 64] = [0u8; 64];
        self.cursor.read_exact(&mut db_name)?;
        self.meta = Some(DbMeta {
            version,
            record_count,
            name: String::from_utf8_lossy(&db_name)
                .trim_matches(char::from(0))
                .to_string(),
        });
        Ok(self)
    }
    pub fn decode_db_file(&mut self) -> PDBResult<&mut Self> {
        self.decode_db_header()?;
        while self.cursor.position() < self.cursor.get_ref().len() as u64 {
            self.decode_db_key_field_pair()?;
        }
        Ok(self)
    }
    pub fn finish(self) -> PDBResult<(HashMap<Key, Field>, DbMeta)> {
        let db = self.db;
        let Some(meta) = self.meta else {
            return Err(PeachDbError::InvalidFinishCall);
        };
        Ok((db, meta))
    }
}
pub struct WalEncoder {
    buffer: Vec<u8>,
}
impl WalEncoder {
    pub fn new() -> Self {
        WalEncoder { buffer: Vec::new() }
    }
    fn write(&mut self, buffer: &[u8]) -> &mut Self {
        self.buffer.extend_from_slice(buffer);
        self
    }

    fn encode_set_bytes(&mut self) -> &mut Self {
        self.write(&[0x1])
    }
    fn encode_delete_bytes(&mut self) -> &mut Self {
        self.write(&[0x2])
    }
    fn encode_key_len(&mut self, key_len: usize) -> &mut Self {
        self.write(&(key_len as u16).to_le_bytes())
    }
    fn encode_key_bytes(&mut self, key: &[u8]) -> &mut Self {
        self.write(key)
    }
    fn encode_key(&mut self, key: &Key) -> &mut Self {
        self.encode_key_len(key.len())
            .encode_key_bytes(key.as_bytes())
    }
    fn encode_field_len(&mut self, field_len: usize) -> &mut Self {
        self.write(&(field_len as u32).to_le_bytes())
    }
    fn encode_field_bytes(&mut self, field: &[u8]) -> &mut Self {
        self.write(field)
    }
    fn encode_field(&mut self, field: &Field) -> &mut Self {
        let payload = bytes_from_field(field);
        self.encode_field_len(payload.len())
            .encode_field_bytes(&payload)
    }
    fn encode_set_entry(&mut self, key: &Key, field: &Field) -> &mut Self {
        self.encode_set_bytes().encode_key(key).encode_field(field)
    }
    fn encode_delete_entry(&mut self, key: &Key) -> &mut Self {
        self.encode_delete_bytes().encode_key(key)
    }
    pub fn encode_wal_entry(&mut self, entry: &WalEntry) -> &mut Self {
        match &entry {
            WalEntry::Set(key, field) => self.encode_set_entry(key, field),
            WalEntry::Delete(key) => self.encode_delete_entry(key),
        }
    }
    pub fn finish(&mut self) -> &[u8] {
        &self.buffer
    }
    pub fn reset(&mut self) {
        self.buffer.clear()
    }
}

impl Default for WalEncoder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WalDecoder<'a> {
    cursor: Cursor<&'a [u8]>,
    wal: Vec<WalEntry>,
}
impl<'a> WalDecoder<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        WalDecoder {
            cursor: Cursor::new(bytes),
            wal: Vec::new(),
        }
    }
    fn wal_push(&mut self, entry: WalEntry) -> &mut Self {
        self.wal.push(entry);
        self
    }
    fn decode_set_entry(&mut self) -> PDBResult<&mut Self> {
        let key_len = self.cursor.read_u16::<LittleEndian>()?;
        let mut key_buf = vec![0u8; key_len as usize];
        self.cursor.read_exact(&mut key_buf)?;
        let field_len: u32 = self.cursor.read_u32::<LittleEndian>()?;
        let mut feld_buf = vec![0u8; field_len as usize];
        self.cursor.read_exact(&mut feld_buf)?;
        let (field, _) = field_from_bytes(&feld_buf)?;

        Ok(self.wal_push(WalEntry::Set(Key::from(key_buf), field)))
    }
    fn decode_delete_entry(&mut self) -> PDBResult<&mut Self> {
        let key_len = self.cursor.read_u16::<LittleEndian>()?;
        let mut key_buf = vec![0u8; key_len as usize];
        self.cursor.read_exact(&mut key_buf)?;
        Ok(self.wal_push(WalEntry::Delete(Key::from(key_buf))))
    }
    pub fn decode_wal_entry(&mut self) -> PDBResult<&mut Self> {
        match self.cursor.read_u8()? {
            0x1 => self.decode_set_entry(),
            0x2 => self.decode_delete_entry(),
            b => Err(PeachDbError::UnknownDtype { byte: b }),
        }
    }
    pub fn decode_wal_entries(&mut self) -> PDBResult<&mut Self> {
        while self.cursor.position() < self.cursor.get_ref().len() as u64 {
            self.decode_wal_entry()?;
        }
        Ok(self)
    }
    pub fn finish(self) -> PDBResult<Vec<WalEntry>> {
        Ok(self.wal)
    }
}

pub struct IndexDecoder<'a> {
    cursor: Cursor<&'a [u8]>,
    index: HashMap<Key, IndexEntry>,
    last_index_key: Vec<u8>,
    last_idx: u64,
    last_key_len: u16,
    last_field_len: u32,
}

impl<'a> IndexDecoder<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        IndexDecoder {
            cursor: Cursor::new(bytes),
            index: HashMap::new(),
            last_index_key: Vec::new(),
            last_idx: 0,
            last_key_len: 0,
            last_field_len: 0,
        }
    }

    fn read_index(&mut self) -> PDBResult<&mut Self> {
        let index = self.cursor.read_u64::<LittleEndian>()?;
        self.last_idx = index;
        Ok(self)
    }
    fn read_key_len(&mut self) -> PDBResult<&mut Self> {
        let key_len = self.cursor.read_u16::<LittleEndian>()?;
        self.last_key_len = key_len;
        Ok(self)
    }
    fn read_key(&mut self) -> PDBResult<&mut Self> {
        let mut key = vec![0u8; self.last_key_len as usize];
        self.cursor.read_exact(&mut key)?;
        self.last_index_key = key;
        Ok(self)
    }
    fn read_field_len(&mut self) -> PDBResult<&mut Self> {
        self.last_field_len = self.cursor.read_u32::<LittleEndian>()?;
        Ok(self)
    }
    fn decode_index_entry(&mut self) -> PDBResult<&mut Self> {
        self.read_key_len()?
            .read_key()?
            .read_field_len()?
            .read_index()?;
        let entry = IndexEntry {
            offset: self.last_idx,
            key_len: self.last_key_len,
            field_len: self.last_field_len,
        };
        self.index
            .insert(Key::from(self.last_index_key.clone()), entry);
        Ok(self)
    }
    pub fn decode_index_file(&mut self) -> PDBResult<&mut Self> {
        while self.cursor.position() < self.cursor.get_ref().len() as u64 {
            self.decode_index_entry()?;
        }
        Ok(self)
    }
    pub fn finish(self) -> PDBResult<HashMap<Key, IndexEntry>> {
        Ok(self.index)
    }
}
pub struct IndexFromDBDecoder<'a> {
    cursor: Cursor<&'a [u8]>,
    index: HashMap<Key, IndexEntry>,
    last_key_len: u16,
    last_field_len: u32,
    last_key: Vec<u8>,
    last_index: u64,
    deleted: bool,
}
impl<'a> IndexFromDBDecoder<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        IndexFromDBDecoder {
            cursor: Cursor::new(bytes),
            index: HashMap::new(),
            last_key: Vec::new(),
            last_field_len: 0,
            last_key_len: 0,
            last_index: 0,
            deleted: false,
        }
    }
    fn read_magic_number(&mut self) -> PDBResult<&mut Self> {
        let mut rec_magic_number = [0u8; 4];
        self.cursor.read_exact(&mut rec_magic_number)?;

        match rec_magic_number {
            [0, 0, 0, 0] => {
                self.deleted = true;
                Ok(self)
            }
            RECORD_MAGIC_NUMBER => {
                self.deleted = false;
                Ok(self)
            }
            _ => Err(PeachDbError::InvalidMagicBytes),
        }
    }
    fn read_key_len(&mut self) -> PDBResult<&mut Self> {
        let mut key_len_bytes = [0u8; 2];
        self.cursor.read_exact(&mut key_len_bytes)?;
        self.last_key_len = u16::from_le_bytes(key_len_bytes);
        Ok(self)
    }
    fn read_field_len(&mut self) -> PDBResult<&mut Self> {
        let mut field_len_bytes = [0u8; 4];
        self.cursor.read_exact(&mut field_len_bytes)?;
        self.last_field_len = u32::from_le_bytes(field_len_bytes);
        Ok(self)
    }
    fn skip_crc(&mut self) -> PDBResult<&mut Self> {
        self.cursor.set_position(self.cursor.position() + 4u64);
        Ok(self)
    }
    fn read_key(&mut self) -> PDBResult<&mut Self> {
        let mut key_bytes = vec![0u8; self.last_key_len as usize];

        self.cursor.read_exact(&mut key_bytes)?;
        self.last_key = key_bytes;
        Ok(self)
    }
    fn skip_field(&mut self) -> PDBResult<&mut Self> {
        self.cursor
            .set_position(self.cursor.position() + self.last_field_len as u64);
        Ok(self)
    }
    fn decode_db_header(&mut self) -> PDBResult<&mut Self> {
        self.cursor.set_position(0);

        self.skip_metadata_key()?;
        self.cursor.read_u64::<LittleEndian>()?;
        self.skip_metadata_key()?;
        self.cursor.read_u8()?;
        self.cursor.read_u8()?;
        self.skip_metadata_key()?;
        self.cursor.read_u64::<LittleEndian>()?;
        self.skip_metadata_key()?;
        self.cursor.set_position(self.cursor.position() + 64u64);
        Ok(self)
    }

    fn skip_metadata_key(&mut self) -> PDBResult<&mut Self> {
        self.cursor.set_position(self.cursor.position() + 8u64);
        Ok(self)
    }
    fn set_last_index(&mut self) -> PDBResult<&mut Self> {
        self.last_index = self.cursor.position();
        Ok(self)
    }
    fn index_push(&mut self) -> PDBResult<&mut Self> {
        if self.deleted {
            return Ok(self);
        }
        if self.last_key.is_empty() {
            return Err(PeachDbError::InvalidIndexEntry);
        }
        let entry = IndexEntry {
            key_len: self.last_key_len,
            offset: self.last_index,
            field_len: self.last_field_len,
        };
        self.index.insert(Key::from(self.last_key.clone()), entry);

        Ok(self)
    }
    pub fn decode_index_from_file(&mut self) -> PDBResult<&mut Self> {
        self.decode_db_header()?;
        while self.cursor.position() < self.cursor.get_ref().len() as u64 {
            self.set_last_index()?
                .read_magic_number()?
                .read_key_len()?
                .read_field_len()?
                .skip_crc()?
                .read_key()?
                .skip_field()?
                .index_push()?;
        }
        Ok(self)
    }
    pub fn finish(self) -> PDBResult<HashMap<Key, IndexEntry>> {
        Ok(self.index)
    }
}

pub struct IndexEncoder {
    buffer: Vec<u8>,
}
impl IndexEncoder {
    pub fn new() -> Self {
        IndexEncoder { buffer: Vec::new() }
    }
    fn write(&mut self, buffer: &[u8]) -> &mut Self {
        self.buffer.extend_from_slice(buffer);
        self
    }
    fn encode_key_len(&mut self, len: u16) -> &mut Self {
        self.write(&len.to_le_bytes())
    }
    fn encode_field_len(&mut self, len: u32) -> &mut Self {
        self.write(&len.to_le_bytes())
    }
    fn encode_key(&mut self, key: &Key) -> &mut Self {
        self.write(key.as_bytes())
    }
    fn encode_index(&mut self, idx: u64) -> &mut Self {
        self.write(&idx.to_le_bytes())
    }
    fn encode_index_entry(&mut self, key: &Key, index: IndexEntry) -> &mut Self {
        self.encode_key_len(index.key_len)
            .encode_key(key)
            .encode_field_len(index.field_len)
            .encode_index(index.offset)
    }
    pub fn encode_index_file(&mut self, index: &HashMap<Key, IndexEntry>) -> &mut Self {
        for (key, idx) in index.iter() {
            self.encode_index_entry(key, idx.clone());
        }
        self
    }
    pub fn finish(&self) -> &[u8] {
        &self.buffer
    }
}

impl Default for IndexEncoder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DBDeleter {
    buffer: Vec<u8>,
    key_len: u16,
    field_len: u32,
}
impl DBDeleter {
    pub fn new() -> Self {
        DBDeleter {
            buffer: Vec::new(),
            key_len: 0,
            field_len: 0,
        }
    }
    pub fn set_key_len(&mut self, len: u16) -> &mut Self {
        self.key_len = len;
        self
    }

    pub fn set_field_len(&mut self, len: u32) -> &mut Self {
        self.field_len = len;
        self
    }
    pub fn get_deleted_field(&mut self) -> &mut Self {
        self.encode_rec_magic()
            .encode_key_len()
            .encode_field_len()
            .encode_crc()
            .encode_key()
            .encode_field()
    }
    pub fn finish(&self) -> &[u8] {
        &self.buffer
    }
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.key_len = 0;
        self.field_len = 0;
    }
    fn write(&mut self, buffer: &[u8]) -> &mut Self {
        self.buffer.extend_from_slice(buffer);
        self
    }
    fn encode_rec_magic(&mut self) -> &mut Self {
        self.write(&[0u8; 4])
    }
    fn encode_key_len(&mut self) -> &mut Self {
        self.write(&self.key_len.to_le_bytes())
    }
    fn encode_field_len(&mut self) -> &mut Self {
        self.write(&self.field_len.to_le_bytes())
    }
    fn encode_crc(&mut self) -> &mut Self {
        self.write(&[0u8; 4])
    }
    fn encode_key(&mut self) -> &mut Self {
        let key = vec![0; self.key_len as usize];
        self.write(&key)
    }
    fn encode_field(&mut self) -> &mut Self {
        let field = vec![0; self.field_len as usize];
        self.write(&field)
    }
}

impl Default for DBDeleter {
    fn default() -> Self {
        Self::new()
    }
}
