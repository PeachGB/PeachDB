use std::path::Path;

use crate::db::codec::{WalDecoder, WalEncoder};
use crate::dtypes::{Field, Key, PDBResult};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

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

pub struct WAL {
    file: File,
    entries: Vec<WalEntry>,
}
impl WAL {
    pub async fn open(path: impl AsRef<Path>) -> PDBResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .append(true)
            .open(path)
            .await?;
        let entries = Vec::new();
        let mut wal = WAL { file, entries };
        wal.replay().await?;
        Ok(wal)
    }
    pub async fn append_entry(&mut self, entry: WalEntry) -> PDBResult<()> {
        let file = &mut self.file;
        let mut encoder = WalEncoder::new();
        let bytes = encoder.encode_wal_entry(&entry).finish();
        file.write_all(bytes).await?;
        self.entries.push(entry);
        file.flush().await?;
        Ok(())
    }
    pub async fn replay(&mut self) -> PDBResult<&[WalEntry]> {
        let file = &mut self.file;
        let mut file_buffer = Vec::new();
        file.seek(std::io::SeekFrom::Start(0)).await?;
        file.read_to_end(&mut file_buffer).await?;
        let mut decoder = WalDecoder::new(&file_buffer);
        self.entries = {
            decoder.decode_wal_entries()?;
            decoder.finish()?
        };
        Ok(&self.entries)
    }
    pub async fn checkpoint(&mut self) -> PDBResult<()> {
        self.entries.clear();
        self.file.set_len(0).await?;
        self.file.seek(std::io::SeekFrom::Start(0)).await?;
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
