mod db;
mod interface;

use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
async fn process_stream(mut input: TcpStream) {
    loop {
        let mut buffer: [u8; 1024] = [0; 1024];
        input.read(&mut buffer).await.unwrap();
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    const IP: &str = "127.0.0.1:8080";

    let server = TcpListener::bind(IP).await?;
    let (stream, _sock_addr) = server.accept().await?;
    tokio::spawn(async move {
        process_stream(stream).await;
    });
    Ok(())
}
