use std::error::Error;

use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom},
    sync::{mpsc, oneshot},
};

use crate::error::PeachDbError;

enum FileCmd {
    ReadAll {
        resp: oneshot::Sender<Result<Vec<u8>, Box<dyn Error + Send + Sync>>>,
    },
    WriteAt {
        pos: u64,
        data: Vec<u8>,
        resp: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    Append {
        data: Vec<u8>,
        resp: oneshot::Sender<Result<u64, Box<dyn Error + Send + Sync>>>,
    },
    SetLen {
        len: u64,
        resp: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    SyncAll {
        resp: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
}

pub struct FileHandler {
    tx: mpsc::Sender<FileCmd>,
}
impl FileHandler {
    pub async fn spawn(mut file: File) -> Result<FileHandler, Box<dyn Error + Send + Sync>> {
        let (tx, mut rx) = mpsc::channel(32);
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    FileCmd::ReadAll { resp } => {
                        let res: Result<Vec<u8>, Box<dyn Error + Send + Sync>> = async {
                            file.seek(SeekFrom::Start(0)).await?;
                            let mut buf = Vec::new();
                            file.read_to_end(&mut buf).await?;
                            Ok(buf)
                        }
                        .await;
                        let _ = resp.send(res);
                    }
                    FileCmd::WriteAt { pos, data, resp } => {
                        let res = async {
                            file.seek(SeekFrom::Start(pos)).await?;
                            file.write_all(&data).await?;
                            Ok(())
                        }
                        .await;
                        let _ = resp.send(res);
                    }
                    FileCmd::Append { data, resp } => {
                        let res: Result<u64, Box<dyn Error + Send + Sync>> = async {
                            let pos = file.seek(SeekFrom::End(0)).await?;
                            file.write_all(&data).await?;
                            Ok(pos)
                        }
                        .await;
                        let _ = resp.send(res);
                    }
                    FileCmd::SetLen { len, resp } => {
                        let res = async {
                            file.set_len(len).await?;
                            Ok(())
                        }
                        .await;
                        let _ = resp.send(res);
                    }

                    FileCmd::SyncAll { resp } => {
                        let res = async {
                            file.sync_all().await?;
                            Ok(())
                        }
                        .await;
                        let _ = resp.send(res);
                    }
                }
            }
        });
        Ok(FileHandler { tx })
    }

    pub async fn read_all(&self) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx.send(FileCmd::ReadAll { resp: resp_tx }).await?;
        resp_rx.await?
    }
    pub async fn write_at(
        &self,
        pos: u64,
        data: Vec<u8>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(FileCmd::WriteAt {
                pos,
                data,
                resp: resp_tx,
            })
            .await?;
        resp_rx.await?
    }
    pub async fn append(&self, data: Vec<u8>) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(FileCmd::Append {
                data,
                resp: resp_tx,
            })
            .await?;
        resp_rx.await?
    }
    pub async fn set_len(&self, len: u64) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx.send(FileCmd::SetLen { len, resp: resp_tx }).await?;
        resp_rx.await?
    }
    pub async fn sync_all(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx.send(FileCmd::SyncAll { resp: resp_tx }).await?;
        resp_rx.await?
    }
}

pub fn map_file_handler_error(err: Box<dyn Error + Send + Sync>) -> PeachDbError {
    PeachDbError::Io(std::io::Error::other(err.to_string()))
}
