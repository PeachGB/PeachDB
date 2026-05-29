use crate::db::Database;
use crate::dtypes::{Key, PDBResult};
use crate::error::PeachDbError;
use crate::server::protocol::{ProtocolDecoder, ProtocolEncoder, Request, Response};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub struct Server {
    db: Arc<Database>,
    addr: String,
}

impl Server {
    pub fn new(addr: &str, db: Database) -> Self {
        Server {
            db: Arc::new(db),
            addr: addr.to_string(),
        }
    }

    pub async fn run(&self) -> PDBResult<()> {
        let listener = TcpListener::bind(&self.addr)
            .await
            .map_err(PeachDbError::Io)?;
        loop {
            let (stream, _addr) = listener.accept().await.map_err(PeachDbError::Io)?;
            let db = Arc::clone(&self.db);
            tokio::spawn(async move {
                let _ = Self::handle_connection(db, stream).await;
            });
        }
    }

    pub(crate) async fn handle_connection(db: Arc<Database>, mut stream: TcpStream) -> PDBResult<()> {
        loop {
            let mut len_buf = [0u8; 4];
            match stream.read_exact(&mut len_buf).await {
                Ok(_) => {}
                Err(e)
                    if e.kind() == std::io::ErrorKind::UnexpectedEof
                        || e.kind() == std::io::ErrorKind::ConnectionReset =>
                {
                    break;
                }
                Err(e) => return Err(PeachDbError::Io(e)),
            }

            let msg_len = u32::from_be_bytes(len_buf) as usize;

            let mut buf = vec![0u8; msg_len];
            if stream.read_exact(&mut buf).await.is_err() {
                Self::send_response(&mut stream, Response::ServerError("failed to read message".into())).await;
                break;
            }

            let req = match ProtocolDecoder::new(&buf).decode_request() {
                Ok(r) => r,
                Err(_) => {
                    if !Self::send_response(&mut stream, Response::InvalidRequest).await {
                        break;
                    }
                    continue;
                }
            };

            let res = Self::handle_request(&db, req).await;
            let mut enc = ProtocolEncoder::new();
            enc.encode_response(&res);
            let framed = enc.frame();
            if stream.write_all(&framed).await.is_err() {
                break;
            }
        }
        Ok(())
    }

    async fn send_response(stream: &mut TcpStream, res: Response) -> bool {
        let mut enc = ProtocolEncoder::new();
        enc.encode_response(&res);
        stream.write_all(&enc.frame()).await.is_ok()
    }

    pub(crate) async fn handle_request(db: &Database, req: Request) -> Response {
        match req {
            Request::Ping => Response::Ok(None),
            Request::Get { key } => match db.get(&key).await {
                Ok(field) => Response::Ok(Some(field)),
                Err(PeachDbError::KeyNotFound) => Response::NotFound,
                Err(e) => Response::ServerError(e.to_string()),
            },
            Request::Set { key, field } => match db.set(key, field).await {
                Ok(()) => Response::Ok(None),
                Err(e) => Response::ServerError(e.to_string()),
            },
            Request::Del { key } => match db.delete(key).await {
                Ok(()) => Response::Ok(None),
                Err(PeachDbError::KeyNotFound) => Response::NotFound,
                Err(e) => Response::ServerError(e.to_string()),
            },
            Request::Keys => match db.keys().await {
                Ok(keys) => Response::Keys(keys),
                Err(e) => Response::ServerError(e.to_string()),
            },
        }
    }
}
