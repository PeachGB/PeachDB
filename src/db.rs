use std::collections::HashMap;
use std::error::Error;
use std::io::SeekFrom;
use tokio::sync::{Mutex, MutexGuard};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};


#[derive(Clone)]
pub enum Primitive{
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

    async fn write_to(&self, file: &mut MutexGuard<'_, File>) -> Result<(), Box<dyn Error>>{
        match self{
            Primitive::Integer(n) =>{
                file.write_i64(*n).await?;
            }
            Primitive::Boolean(bool) => {
                
            }
            Primitive::String(buffer) => {
                
            }
            Primitive::Float(f) => {
                
            }
            
        }
        Ok(())
    }
}


pub enum Dtype{
    Integer,
    Float,
    String,
    Boolean,
    Map,
    Array
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
            _ => panic!("Invalid Dtype")
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
    Map(HashMap<[char;16], Primitive>),
    Array([Primitive; 1024])

}
impl Field {
    pub fn size(&self) -> u32{
        match self{
            Field::Primitive(p) => p.size(),
            Field::Map(m) => {
                let mut size = 0;
                for (_,v) in m.iter() {
                    size += v.size();
                }
                size
            },
            Field::Array(a) => {
                let mut size = 0;
                for v in a.iter() {
                    size += v.size();
                }
                size
            }
        }
    }
    pub fn dtype(&self) -> Dtype{
        match self{
            Field::Primitive(p) => match p {
                Primitive::Integer(_) => Dtype::Integer,
                Primitive::Float(_) => Dtype::Float,
                Primitive::String(_) => Dtype::String,
                Primitive::Boolean(_) => Dtype::Boolean,
            },
            Field::Map(_) => Dtype::Map,
            Field::Array(_) => Dtype::Array
        }
    }
}
pub struct DataBase{
    name: [char; 16],
    fields: Mutex<HashMap<[char;16], Field>>,
    file: Mutex<File>,
    index: Mutex<HashMap<String,u64>>
}
impl DataBase {

    pub async fn new(name: &str) -> Result<DataBase, Box<dyn Error>>{
        let mut n:[char;16] = [' '; 16];
        if name.len() > 16 {
            return Err("Name too long".into());
        }
        for (i,c) in name.chars().enumerate() {
            if c == '\0' {
                return Err("Name contains null".into());
            }
            n[i] = c;
        }

        let name = n.clone().iter().collect::<String>() + ".pdb";
        let mut file = File::open(name).await?;
        let header = file.stream_position().await?;
        file.write_u64(0x50656163684442).await?;
        let version = file.stream_position().await?;
        file.write_u8(1).await?;
        let record_count = file.stream_position().await?;
        file.write_u64(0).await?;
        let db_name = file.stream_position().await?;
        for i in 0..n.len() {
            file.write_u8(n[i] as u8).await?;
        }
        
        let mut index = HashMap::new();
        index.insert("Header".to_string(), header);
        index.insert("version".to_string(), version);
        index.insert("record_count".to_string(), record_count);
        index.insert("db_name".to_string(), db_name);
        
        
        Ok(DataBase{
            name: n,
            fields: Mutex::from(HashMap::new()),
            file:Mutex::from(file),
            index: Mutex::from(index),
        })

    }
    pub async fn insert(&mut self, key: [char;16], value:Field) -> Result<(), Box<dyn Error>>{
        let mut file = self.file.lock().await;
        let mut index = self.index.lock().await;
        let mut fields = self.fields.lock().await;
        file.seek(SeekFrom::End(0)).await?;
        let current = file.stream_position().await?;
        let field_type = value.dtype().as_byte();
        let field_size = value.size();
        let id = key.clone().iter().collect::<String>();
        index.insert(id, current);
        fields.insert(key, value.clone());
        
        file.write_u8('K' as u8).await?;
        file.write_u8('E' as u8).await?;
        file.write_u8('Y' as u8).await?;
        file.write_u8(' ' as u8).await?;
        for i in 0..key.len() {
            file.write_u8(key[i] as u8).await?;
        }
        file.write_u8(field_type).await?;
        file.write_u32(field_size).await?;
        
        match value {
            Field::Primitive(a) =>{
                a.write_to(&mut file)
            }
        }








        Ok(())
    }
    pub async fn update_record_count_(&mut self) -> Result<(), Box<dyn Error>>{
        let mut file = self.file.lock().await;
        let mut current = file.stream_position().await?;
        let mut index = self.index.lock().await;
        file.seek(SeekFrom::Start(index["record_count"])).await?;
        let count = file.read_u64().await?;
        file.write_u64(count + 1).await?;
        file.seek(SeekFrom::Start(current)).await?;
        Ok(())

    }

}
