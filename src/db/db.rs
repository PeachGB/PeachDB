use std::{collections::HashMap, sync::Arc};

use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Mutex, RwLock},
};

use crate::{
    db::{
        DbMeta,
        codec::{DBDecoder, DBDeleter, DBEncoder, IndexDecoder, IndexEncoder, IndexFromDBDecoder, encode},
        file::FileHandler,
        wal::{WAL, WalEntry},
    },
    dtypes::{Field, Key, PDBResult, bytes_from_field},
    error::PeachDbError,
};

fn map_file_handler_error(err: Box<dyn std::error::Error + Send + Sync>) -> PeachDbError {
    PeachDbError::Io(std::io::Error::other(err.to_string()))
}

#[derive(Clone, Hash)]
pub struct IndexEntry {
    pub offset: u64,
    pub key_len: u16,
    pub field_len: u32,
}
pub struct State {
    fields: HashMap<Key, Field>,
    //offset,keylen,fieldlen
    index: HashMap<Key, IndexEntry>,
}

pub struct Database {
    state: Arc<RwLock<State>>,
    wal: Mutex<WAL>,
    file: FileHandler,
    index_file: FileHandler,
    meta: DbMeta,
}
async fn is_new_file(file: &File) -> PDBResult<bool> {
    Ok(file.metadata().await?.len() == 0)
}
impl Database {
    async fn new(name: String, mut file: File, mut index_file: File, wal: WAL) -> PDBResult<Self> {
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

        let file = FileHandler::spawn(file)
            .await
            .map_err(|e| PeachDbError::Io(std::io::Error::other(e.to_string())))?;
        let index_file = FileHandler::spawn(index_file)
            .await
            .map_err(|e| PeachDbError::Io(std::io::Error::other(e.to_string())))?;

        Ok(Database {
            state: Arc::new(RwLock::new(State {
                fields: HashMap::new(),
                index: HashMap::new(),
            })),
            wal: Mutex::new(wal),
            file,
            index_file,
            meta,
        })
    }
    async fn build_index_from_file(
        name: &String,
        file: &mut File,
    ) -> PDBResult<HashMap<Key, IndexEntry>> {
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
        let file = FileHandler::spawn(file)
            .await
            .map_err(map_file_handler_error)?;
        let index_file = FileHandler::spawn(index_file)
            .await
            .map_err(map_file_handler_error)?;

        Ok(Database {
            state: Arc::new(RwLock::new(State {
                fields: fields,
                index: index,
            })),
            wal: Mutex::new(wal),
            file,
            index_file,
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
        {
            self.wal
                .lock()
                .await
                .append_entry(WalEntry::Set(key.clone(), field.clone()))
                .await?;
        }
        {
            let mut db = self.state.write().await;
            db.fields.insert(key, field);
        }
        Ok(())
    }
    pub async fn delete(&self, key: Key) -> PDBResult<()> {
        {
            self.wal
                .lock()
                .await
                .append_entry(WalEntry::Delete(key.clone()))
                .await?;
        }
        {
            let mut db = self.state.write().await;
            db.fields.remove(&key);
        }
        Ok(())
    }
    pub async fn flush(&self) -> PDBResult<()> {
        use WalEntry::*;
        let entries = self.wal.lock().await.pop_uncommited_entries().await?;
        let mut deleter = DBDeleter::new();
        let mut delete_plan: HashMap<Key, IndexEntry> = HashMap::new();
        {
            let state = self.state.read().await;
            for entry in &entries {
                if let Delete(key) = entry {
                    if let Some(index_entry) = state.index.get(key) {
                        delete_plan.insert(key.clone(), index_entry.clone());
                    }
                }
            }
        }
        let mut indexes: Vec<(Key, IndexEntry)> = Vec::new();
        for entry in entries.into_iter() {
            match entry {
                Set(key, field) => {
                    let record = encode(&key, &field);
                    let record_start = self
                        .file
                        .append(record)
                        .await
                        .map_err(map_file_handler_error)?;
                    indexes.push((
                        key.clone(),
                        IndexEntry {
                            offset: record_start,
                            key_len: key.len() as u16,
                            field_len: bytes_from_field(&field).len() as u32,
                        },
                    ));
                }
                Delete(key) => {
                    let Some(entry) = delete_plan.get(&key) else {
                        continue;
                    };
                    let deleted = deleter
                        .set_key_len(entry.key_len)
                        .set_field_len(entry.field_len)
                        .get_deleted_field()
                        .finish();
                    self.file
                        .write_at(entry.offset, deleted.to_vec())
                        .await
                        .map_err(map_file_handler_error)?;
                    deleter.reset();
                }
                Commit => unreachable!(),
            }
        }
        self.file.sync_all().await.map_err(map_file_handler_error)?;
        let mut state = self.state.write().await;
        for (k, i) in indexes.into_iter() {
            state.index.insert(k, i);
        }
        for key in delete_plan.keys() {
            state.index.remove(key);
            state.fields.remove(key);
        }
        drop(state);
        self.rebuild_index().await?;
        self.wal.lock().await.checkpoint().await?;

        Ok(())
    }
    pub async fn keys(&self) -> PDBResult<Vec<Key>> {
        let state = self.state.read().await;
        Ok(state.fields.keys().cloned().collect())
    }

    async fn rebuild_index(&self) -> PDBResult<()> {
        let mut encoder = IndexEncoder::new();

        let index = {
            let state = self.state.read().await;
            state.index.clone()
        };
        encoder.encode_index_file(&index);
        self.index_file
            .set_len(0)
            .await
            .map_err(map_file_handler_error)?;
        self.index_file
            .write_at(0, encoder.finish().to_vec())
            .await
            .map_err(map_file_handler_error)?;
        self.index_file
            .sync_all()
            .await
            .map_err(map_file_handler_error)?;

        Ok(())
    }
}
