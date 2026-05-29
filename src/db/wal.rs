use std::path::Path;

use crate::{
    db::{
        codec::{WalDecoder, WalEncoder},
        file::{FileHandler, map_file_handler_error},
    },
    dtypes::{Field, Key, PDBResult},
};
use tokio::fs::OpenOptions;

#[derive(Clone)]
pub enum WalEntry {
    Set(Key, Field),
    Delete(Key),
    //Action has been commited
    Commit,
}
impl WalEntry {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            WalEntry::Set(_, _) => &[0x1],
            WalEntry::Delete(_) => &[0x2],
            WalEntry::Commit => b"COMMIT--",
        }
    }
}

pub struct Wal {
    file: FileHandler,
    entries: Vec<WalEntry>,
}
impl Wal {
    pub async fn open(path: impl AsRef<Path>) -> PDBResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .append(true)
            .open(path)
            .await?;
        let file = FileHandler::spawn(file)
            .await
            .map_err(map_file_handler_error)?;
        let entries = Vec::new();
        let mut wal = Wal { file, entries };
        wal.replay().await?;
        Ok(wal)
    }
    pub async fn append_entry(&mut self, entry: WalEntry) -> PDBResult<()> {
        let mut encoder = WalEncoder::new();
        let bytes = encoder.encode_wal_entry(&entry).finish().to_vec();
        self.file
            .append(bytes)
            .await
            .map_err(map_file_handler_error)?;
        self.entries.push(entry);
        self.file.sync_all().await.map_err(map_file_handler_error)?;
        Ok(())
    }
    pub async fn replay(&mut self) -> PDBResult<&[WalEntry]> {
        let file_buffer = self.file.read_all().await.map_err(map_file_handler_error)?;
        let mut decoder = WalDecoder::new(&file_buffer);
        self.entries = {
            decoder.decode_wal_entries()?;
            decoder.finish()?
        };
        Ok(&self.entries)
    }
    pub async fn checkpoint(&mut self) -> PDBResult<()> {
        self.entries.clear();
        self.file.set_len(0).await.map_err(map_file_handler_error)?;
        self.file.sync_all().await.map_err(map_file_handler_error)?;
        Ok(())
    }
    pub async fn pop_uncommited_entries(&self) -> PDBResult<Vec<WalEntry>> {
        let mut uncommited: Vec<WalEntry> = Vec::new();
        for entry in self.entries.iter().rev() {
            match entry {
                WalEntry::Commit => break,
                WalEntry::Set(_, _) => uncommited.push(entry.clone()),
                WalEntry::Delete(_) => uncommited.push(entry.clone()),
            }
        }
        uncommited.reverse();
        Ok(uncommited)
    }
}
