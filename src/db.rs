use std::{collections::HashMap, error::Error, io::SeekFrom};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, MutexGuard};

static HEADER: u64 = 0x50656163684442;
static VERSION: u8 = 1;
const HEADER_KEY: [u8; 16] = *b"Header          ";
const VERSION_KEY: [u8; 16] = *b"version         ";
const RECORD_COUNT_KEY: [u8; 16] = *b"record_count    ";
const DB_NAME_KEY: [u8; 16] = *b"db_name         ";

fn is_metadata_key(key: &[u8; 16]) -> bool {
    key == &HEADER_KEY || key == &VERSION_KEY || key == &RECORD_COUNT_KEY || key == &DB_NAME_KEY
}

pub trait Writeable {
    async fn write_to(&self, file: &mut MutexGuard<'_, File>) -> Result<(), Box<dyn Error>>;
}

#[derive(Clone)]
pub enum Primitive {
    Integer(i64),
    Float(f64),
    String([char; 65535]),
    Boolean(bool),
}

impl Primitive {
    pub fn size(&self) -> u32 {
        match self {
            Primitive::Integer(n) => 8,
            Primitive::Float(n) => 8,
            Primitive::String(s) => 65535,
            Primitive::Boolean(b) => 1,
        }
    }
}

impl Writeable for Primitive {
    async fn write_to(&self, file: &mut MutexGuard<'_, File>) -> Result<(), Box<dyn Error>> {
        match self {
            Primitive::Integer(n) => {
                file.write_i64(*n).await?;
            }
            Primitive::Boolean(bool) => {
                file.write_u8(*bool as u8).await?;
            }
            Primitive::String(buffer) => {
                file.write_u8(buffer.len() as u8).await?;
                for c in buffer.iter() {
                    file.write_u8(*c as u8).await?;
                }
            }
            Primitive::Float(f) => {
                file.write_f64(*f).await?;
            }
        }
        Ok(())
    }
}

pub enum Dtype {
    Integer,
    Float,
    String,
    Boolean,
    Map,
    Array,
}
impl Dtype {
    pub fn from_byte(b: u8) -> Dtype {
        match b {
            0 => Dtype::Integer,
            1 => Dtype::Float,
            2 => Dtype::String,
            3 => Dtype::Boolean,
            4 => Dtype::Map,
            5 => Dtype::Array,
            _ => panic!("Invalid Dtype"),
        }
    }
    pub fn as_byte(&self) -> u8 {
        match self {
            Dtype::Integer => 0,
            Dtype::Float => 1,
            Dtype::String => 2,
            Dtype::Boolean => 3,
            Dtype::Map => 4,
            Dtype::Array => 5,
        }
    }
}

#[derive(Clone)]
pub enum Field {
    Primitive(Primitive),
    Map(HashMap<[char; 16], Primitive>),
    Array([Primitive; 1024]),
}
impl Writeable for Field {
    async fn write_to(&self, file: &mut MutexGuard<'_, File>) -> Result<(), Box<dyn Error>> {
        match self {
            Field::Primitive(p) => {
                p.write_to(file).await?;
            }
            Field::Map(m) => {
                for (k, v) in m.iter() {
                    file.write_u8('K' as u8).await?;
                    file.write_u8('E' as u8).await?;
                    file.write_u8('Y' as u8).await?;
                    file.write_u8('F' as u8).await?;
                    for i in 0..k.len() {
                        file.write_u8(k[i] as u8).await?;
                    }
                    v.write_to(file).await?;
                }
            }
            Field::Array(a) => {
                for v in a.iter() {
                    v.write_to(file).await?;
                }
            }
        }
        Ok(())
    }
}
impl Field {
    pub fn size(&self) -> u32 {
        match self {
            Field::Primitive(p) => p.size(),
            Field::Map(m) => {
                let mut size = 0;
                for (_, v) in m.iter() {
                    size += v.size();
                }
                size
            }
            Field::Array(a) => {
                let mut size = 0;
                for v in a.iter() {
                    size += v.size();
                }
                size
            }
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
            Field::Map(_) => Dtype::Map,
            Field::Array(_) => Dtype::Array,
        }
    }
}
pub struct DataBase {
    name: [u8; 16],
    fields: Mutex<HashMap<[u8; 16], Field>>,
    file: Mutex<File>,
    index: Mutex<HashMap<[u8; 16], u64>>,
    index_file: Mutex<File>,
}

impl DataBase {
    pub async fn new(name: &str) -> Result<DataBase, Box<dyn Error>> {
        let n: [u8; 16] = name.as_bytes().try_into().expect("invalid name");

        let base_name = name.to_string();
        let name = format!("{}.pdb", base_name);
        let index_name = format!("{}.pdi", base_name);

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&name)
            .await?;
        let index_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&index_name)
            .await?;

        let is_new = file.metadata().await?.len() == 0;
        let mut index: HashMap<[u8; 16], u64> = HashMap::new();
        if is_new {
            let header = file.stream_position().await?;
            file.write_u64(HEADER).await?;
            let version = file.stream_position().await?;
            file.write_u8(VERSION).await?;
            let record_count = file.stream_position().await?;
            file.write_u64(0).await?;
            let db_name = file.stream_position().await?;
            for i in 0..n.len() {
                file.write_u8(n[i] as u8).await?;
            }

            index.insert(HEADER_KEY, header);
            index.insert(VERSION_KEY, version);
            index.insert(RECORD_COUNT_KEY, record_count);
            index.insert(DB_NAME_KEY, db_name);
        }

        let mut db = DataBase {
            name: n,
            fields: Mutex::from(HashMap::new()),
            file: Mutex::from(file),
            index: Mutex::from(index),
            index_file: Mutex::from(index_file),
        };
        if is_new {
            db.index_persist().await?;
        } else {
            db.load_index().await?;
            if !db.index.lock().await.is_empty() {
                db.load().await?;
            }
        }

        Ok(db)
    }
    pub async fn index_persist(&self) -> Result<(), Box<dyn Error>> {
        let mut index_file = self.index_file.lock().await;
        let index = self.index.lock().await;
        index_file.set_len(0).await?;
        index_file.seek(SeekFrom::Start(0)).await?;

        for (k, v) in index.iter() {
            index_file.write_all(k).await?;
            index_file.write_u64(*v).await?;
        }
        index_file.flush().await?;
        Ok(())
    }
    pub async fn load_index(&self) -> Result<(), Box<dyn Error>> {
        let mut index_file = self.index_file.lock().await;
        let mut index: HashMap<[u8; 16], u64> = HashMap::new();
        index_file.seek(SeekFrom::Start(0)).await?;
        let mut buffer = [0u8; 24];
        loop {
            match index_file.read_exact(&mut buffer).await {
                Ok(_) => {
                    let key: [u8; 16] = buffer[0..16].try_into().unwrap();
                    let value = u64::from_le_bytes(buffer[16..24].try_into().unwrap());
                    index.insert(key, value);
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    } else {
                        return Err(Box::new(e));
                    }
                }
            }
        }
        let mut index_lock = self.index.lock().await;
        *index_lock = index;
        Ok(())
    }
    pub async fn get(&self, key: [u8; 16]) -> Option<Field> {
        let fields = self.fields.lock().await;
        fields.get(&key).cloned()
    }
    pub async fn insert(&mut self, key: [u8; 16], value: Field) -> Result<(), Box<dyn Error>> {
        let mut fields = self.fields.lock().await;
        fields.insert(key, value);
        Ok(())
    }
    pub async fn load(&mut self) -> Result<(), Box<dyn Error>> {
        let mut file = self.file.lock().await;
        let index = self.index.lock().await;
        for (key, pos) in index.iter() {
            if is_metadata_key(key) {
                continue;
            }
            file.seek(SeekFrom::Start(*pos)).await?;
            let mut buffer = [0u8; 4];
            file.read_exact(&mut buffer).await?;
            if &buffer == b"KEY " {
                let mut key_buffer = [0u8; 16];
                file.read_exact(&mut key_buffer).await?;
                let field_type = file.read_u8().await?;
                let _field_size = file.read_u32().await?;
                let value = match Dtype::from_byte(field_type) {
                    Dtype::Integer => {
                        let n = file.read_i64().await?;
                        Field::Primitive(Primitive::Integer(n))
                    }
                    Dtype::Float => {
                        let f = file.read_f64().await?;
                        Field::Primitive(Primitive::Float(f))
                    }
                    Dtype::String => {
                        let len = file.read_u8().await? as usize;
                        let mut s: [char; 65535] = ['\0'; 65535];
                        for i in 0..len {
                            s[i] = file.read_u8().await? as char;
                        }
                        Field::Primitive(Primitive::String(s))
                    }
                    Dtype::Boolean => {
                        let b = file.read_u8().await? != 0;
                        Field::Primitive(Primitive::Boolean(b))
                    }
                    _ => panic!("Invalid Dtype"),
                };
                let mut fields_lock = self.fields.lock().await;
                fields_lock.insert(*key, value);
            } else {
                panic!("Invalid record");
            }
        }
        Ok(())
    }

    pub async fn persist(&mut self, key: [u8; 16], value: Field) -> Result<(), Box<dyn Error>> {
        let mut file = self.file.lock().await;
        let mut fields = self.fields.lock().await;
        file.seek(SeekFrom::End(0)).await?;
        let current = file.stream_position().await?;
        let field_type = value.dtype().as_byte();
        let field_size = value.size();
        let id = key.clone().map(|c| c as u8);
        {
            let mut index = self.index.lock().await;
            index.insert(id, current);
        }
        self.index_persist().await?;

        if !fields.contains_key(&id) {
            fields.insert(key, value.clone());
        }
        file.write_u8('K' as u8).await?;
        file.write_u8('E' as u8).await?;
        file.write_u8('Y' as u8).await?;
        file.write_u8(' ' as u8).await?;
        for i in 0..key.len() {
            file.write_u8(key[i] as u8).await?;
        }
        file.write_u8(field_type).await?;
        file.write_u32(field_size).await?;

        value.write_to(&mut file).await?;

        file.write_u8('E' as u8).await?;
        file.write_u8('N' as u8).await?;
        file.write_u8('D' as u8).await?;
        file.write_u8(' ' as u8).await?;
        file.flush().await?;
        drop(file);
        drop(fields);
        self.update_record_count_().await?;
        Ok(())
    }

    pub async fn flush(&self) -> Result<(), Box<dyn Error>> {
        self.index_persist().await?;
        let mut file = self.file.lock().await;
        file.flush().await?;
        Ok(())
    }

    pub async fn update_record_count_(&mut self) -> Result<(), Box<dyn Error>> {
        let mut file = self.file.lock().await;
        let current = file.stream_position().await?;
        let index = self.index.lock().await;
        file.seek(SeekFrom::Start(index[&RECORD_COUNT_KEY])).await?;
        let count = file.read_u64().await?;
        file.write_u64(count + 1).await?;
        file.seek(SeekFrom::Start(current)).await?;
        Ok(())
    }
}
