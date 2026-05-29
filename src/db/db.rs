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
        wal::{Wal, WalEntry},
    },
    dtypes::{Field, Key, PDBResult, bytes_from_field},
    error::PeachDbError,
};

fn map_file_handler_error(err: Box<dyn std::error::Error + Send + Sync>) -> PeachDbError {
    PeachDbError::Io(std::io::Error::other(err.to_string()))
}

// Header layout: Header(8)+magic(8)+Version(8)+ver(1)+ver_dec(1)+RecCount(8) = 34
const RECORD_COUNT_OFFSET: u64 = 34;

#[derive(Clone, Hash)]
pub struct IndexEntry {
    pub offset: u64,
    pub key_len: u16,
    pub field_len: u32,
}

pub struct State {
    fields: HashMap<Key, Field>,
    index: HashMap<Key, IndexEntry>,
}

pub struct Database {
    state: Arc<RwLock<State>>,
    wal: Mutex<Wal>,
    file: FileHandler,
    index_file: FileHandler,
}

async fn is_new_file(file: &File) -> PDBResult<bool> {
    Ok(file.metadata().await?.len() == 0)
}

impl Database {
    async fn new(name: String, mut file: File, index_file: File, wal: Wal) -> PDBResult<Self> {
        let meta = DbMeta { name, version: 1, record_count: 0 };
        file.set_len(0).await?;
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
        })
    }

    async fn init(mut file: File, mut index_file: File, wal: Wal) -> PDBResult<Self> {
        let mut file_buf = Vec::with_capacity(file.metadata().await?.len() as usize);
        file.read_to_end(&mut file_buf).await?;
        let mut index_file_buf = Vec::with_capacity(index_file.metadata().await?.len() as usize);
        index_file.read_to_end(&mut index_file_buf).await?;

        let mut decoder = DBDecoder::new(&file_buf);
        let (mut fields, _) = {
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

        for entry in wal.pop_uncommited_entries().await? {
            match entry {
                WalEntry::Set(key, field) => { fields.insert(key, field); }
                WalEntry::Delete(key) => { fields.remove(&key); }
            }
        }

        let file = FileHandler::spawn(file)
            .await
            .map_err(map_file_handler_error)?;
        let index_file = FileHandler::spawn(index_file)
            .await
            .map_err(map_file_handler_error)?;

        Ok(Database {
            state: Arc::new(RwLock::new(State { fields, index })),
            wal: Mutex::new(wal),
            file,
            index_file,
        })
    }
}

impl Database {
    pub async fn open(name: String) -> PDBResult<Self> {
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
        let wal = Wal::open(format!("{}.wal", name)).await?;

        match (db_is_new, index_is_new) {
            (true, _) => Database::new(name, file, index_file, wal).await,
            (false, _) => Database::init(file, index_file, wal).await,
        }
    }

    pub async fn get(&self, key: &Key) -> PDBResult<Field> {
        let state = self.state.read().await;
        if let Some(field) = state.fields.get(key) {
            Ok(field.clone())
        } else {
            Err(PeachDbError::KeyNotFound)
        }
    }

    pub async fn set(&self, key: Key, field: Field) -> PDBResult<()> {
        self.wal
            .lock()
            .await
            .append_entry(WalEntry::Set(key.clone(), field.clone()))
            .await?;
        self.state.write().await.fields.insert(key, field);
        Ok(())
    }

    pub async fn delete(&self, key: Key) -> PDBResult<()> {
        self.wal
            .lock()
            .await
            .append_entry(WalEntry::Delete(key.clone()))
            .await?;
        self.state.write().await.fields.remove(&key);
        Ok(())
    }

    pub async fn flush(&self) -> PDBResult<()> {
        use WalEntry::*;
        let entries = self.wal.lock().await.pop_uncommited_entries().await?;

        // Deduplicate: if the same key appears multiple times, only the last
        // operation matters. This prevents a Set followed by a Delete in the
        // same batch from leaving an orphaned (un-tombstoned) record on disk.
        let mut net: HashMap<Key, WalEntry> = HashMap::new();
        for entry in entries {
            let key = match &entry {
                Set(k, _) => k.clone(),
                Delete(k) => k.clone(),
            };
            net.insert(key, entry);
        }

        let mut deleter = DBDeleter::new();
        let mut delete_plan: HashMap<Key, IndexEntry> = HashMap::new();
        {
            let state = self.state.read().await;
            for (key, entry) in &net {
                if let Delete(_) = entry {
                    if let Some(index_entry) = state.index.get(key) {
                        delete_plan.insert(key.clone(), index_entry.clone());
                    }
                }
            }
        }
        let mut indexes: Vec<(Key, IndexEntry)> = Vec::new();
        for entry in net.into_values() {
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
        self.update_record_count().await?;
        self.rebuild_index().await?;
        self.wal.lock().await.checkpoint().await?;
        Ok(())
    }

    async fn update_record_count(&self) -> PDBResult<()> {
        let count = self.state.read().await.fields.len() as u64;
        self.file
            .write_at(RECORD_COUNT_OFFSET, count.to_le_bytes().to_vec())
            .await
            .map_err(map_file_handler_error)
    }

    pub async fn keys(&self) -> PDBResult<Vec<Key>> {
        Ok(self.state.read().await.fields.keys().cloned().collect())
    }

    async fn rebuild_index(&self) -> PDBResult<()> {
        let mut encoder = IndexEncoder::new();
        let index = self.state.read().await.index.clone();
        encoder.encode_index_file(&index);
        self.index_file.set_len(0).await.map_err(map_file_handler_error)?;
        self.index_file
            .write_at(0, encoder.finish().to_vec())
            .await
            .map_err(map_file_handler_error)?;
        self.index_file.sync_all().await.map_err(map_file_handler_error)?;
        Ok(())
    }
}
