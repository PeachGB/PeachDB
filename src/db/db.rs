use std::{collections::HashMap, io::SeekFrom, sync::Arc};

use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::{Mutex, RwLock},
};

use crate::{
    db::{
        DbMeta,
        codec::{DBDecoder, DBDeleter, DBEncoder, IndexDecoder, IndexEncoder, IndexFromDBDecoder},
        wal::{WAL, WalEntry},
    },
    dtypes::{Field, Key, PDBResult},
    error::PeachDbError,
};

pub struct State {
    fields: HashMap<Key, Field>,
    index: HashMap<Key, u64>,
}

pub struct Database {
    state: Arc<RwLock<State>>,
    wal: Mutex<WAL>,
    file: Mutex<File>,
    index_file: Mutex<File>,
    meta: DbMeta,
}
async fn is_new_file(file: &File) -> PDBResult<bool> {
    Ok(file.metadata().await?.len() == 0)
}
impl Database {
    async fn new(name: String, mut file: File, index_file: File, wal: WAL) -> PDBResult<Self> {
        let meta = DbMeta {
            name: name,
            version: 1,
            record_count: 0,
        };
        file.set_len(0).await?;
        index_file.set_len(0).await?;
        let mut encoder = DBEncoder::new();
        encoder.encode_db_header(&meta);
        let header = encoder.finish();
        file.write_all(header).await?;

        Ok(Database {
            state: Arc::new(RwLock::new(State {
                fields: HashMap::new(),
                index: HashMap::new(),
            })),
            wal: Mutex::new(wal),
            file: Mutex::new(file),
            index_file: Mutex::new(index_file),
            meta,
        })
    }
    async fn build_index_from_file(name: &String, file: &mut File) -> PDBResult<HashMap<Key, u64>> {
        let mut idx: File = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(format!("{}.dbidx", name))
            .await?;
        let mut file_buf = Vec::new();
        file.read_to_end(&mut file_buf).await?;
        let mut decoder = IndexFromDBDecoder::new(&file_buf);
        decoder.decode_index_from_file()?;
        let index = decoder.finish()?;
        let mut encoder = IndexEncoder::new();
        encoder.encode_index_file(&index);
        idx.write_all(&encoder.finish()).await?;
        Ok(index)
    }
    async fn init(mut file: File, mut index_file: File, wal: WAL) -> PDBResult<Self> {
        let mut file_buf = Vec::with_capacity(file.metadata().await?.len() as usize);

        file.read_to_end(&mut file_buf).await?;
        let mut index_file_buf = Vec::with_capacity(index_file.metadata().await?.len() as usize);
        index_file.read_to_end(&mut index_file_buf).await?;
        let mut decoder = DBDecoder::new(&file_buf);

        let (fields, meta) = {
            decoder.decode_db_file()?;
            decoder.finish()?
        };
        let index = if !index_file_buf.is_empty() {
            let mut index_decoder = IndexDecoder::new(&index_file_buf);
            index_decoder.decode_index_file()?;
            index_decoder.finish()?
        } else {
            let mut index_decoder = IndexFromDBDecoder::new(&file_buf);
            index_decoder.decode_index_from_file()?;
            index_decoder.finish()?
        };
        Ok(Database {
            state: Arc::new(RwLock::new(State {
                fields: fields,
                index: index,
            })),
            wal: Mutex::new(wal),
            file: Mutex::new(file),
            index_file: Mutex::new(index_file),
            meta,
        })
    }
}
impl Database {
    pub async fn open(name: String) -> PDBResult<Self> {
        //db name is 64Bytes
        if name.len() > 64 {
            return Err(PeachDbError::DBNameToLong);
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(format!("{}.db", name))
            .await?;
        let index_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(format!("{}.dbidx", name))
            .await?;
        let db_is_new = is_new_file(&file).await?;
        let index_is_new = is_new_file(&index_file).await?;
        let wal = WAL::open(format!("{}.wal", name)).await?;

        match (db_is_new, index_is_new) {
            (true, true) => Database::new(name, file, index_file, wal).await,
            (false, false) => Database::init(file, index_file, wal).await,
            (true, false) => Database::new(name, file, index_file, wal).await,
            (false, true) => Database::init(file, index_file, wal).await,
        }
    }

    pub async fn get(&self, key: &Key) -> PDBResult<Field> {
        let state = self.state.read().await;
        match state.fields.contains_key(key) {
            true => {
                let field = state.fields.get(key).unwrap_or_else(|| unreachable!());
                Ok(field.clone())
            }
            false => {
                //todo! make fn to take key,field from wal
                return Err(PeachDbError::KeyNotFound);
            }
        }
    }

    pub async fn set(&self, key: Key, field: Field) -> PDBResult<()> {
        self.wal
            .lock()
            .await
            .append_entry(WalEntry::Set(key.clone(), field.clone()))
            .await?;
        let mut db = self.state.write().await;
        db.fields.insert(key, field);
        Ok(())
    }
    pub async fn delete(&self, key: Key) -> PDBResult<()> {
        self.wal
            .lock()
            .await
            .append_entry(WalEntry::Delete(key.clone()))
            .await?;
        let mut db = self.state.write().await;
        db.fields.remove(&key);
        Ok(())
    }
    pub async fn flush(&self) -> PDBResult<()> {
        use WalEntry::*;
        let entries = self.wal.lock().await.pop_uncommited_entries().await?;
        let mut encoder = DBEncoder::new();
        let mut deleter = DBDeleter::new();
        let mut file = self.file.lock().await;
        let mut state = self.state.write().await;
        let initial_offset = file.seek(SeekFrom::End(0)).await?;
        let mut offsets: Vec<(Key, u64)> = Vec::new();
        for entry in entries.into_iter() {
            match entry {
                Set(key, field) => {
                    let record_start = initial_offset + encoder.finish().len() as u64;
                    encoder.encode_db_key_field_pair(&key, &field);
                    offsets.push((key, record_start));
                }
                Delete(key) => {
                    let Some(pos) = state.index.get(&key) else {
                        continue;
                    };
                    file.seek(SeekFrom::Start(pos.clone() + 4)).await?;
                    let key_len = file.read_u16().await?;
                    let field_len = file.read_u32().await?;
                    let deleted = deleter
                        .set_key_len(key_len)
                        .set_field_len(field_len)
                        .get_deleted_field()
                        .finish();
                    file.seek(SeekFrom::Start(pos.clone())).await?;
                    file.write_all(deleted).await?;
                    deleter.reset();
                }
                Commit => unreachable!(),
            }
        }
        file.seek(SeekFrom::End(0)).await?;
        if !encoder.finish().is_empty() {
            file.write_all(encoder.finish()).await?;
        }

        file.flush().await?;
        for (k, i) in offsets.into_iter() {
            state.index.insert(k, i);
        }
        drop(state);
        self.rebuild_index().await?;

        Ok(())
    }
    async fn rebuild_index(&self) -> PDBResult<()> {
        let mut encoder = IndexEncoder::new();

        let index = &self.state.read().await.index;
        let mut index_file = self.index_file.lock().await;
        encoder.encode_index_file(index);
        index_file.seek(SeekFrom::Start(0)).await?;
        index_file.write_all(encoder.finish()).await?;
        index_file.flush().await?;

        Ok(())
    }
}
