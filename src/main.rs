mod db;
mod dtypes;
mod error;
mod server;
#[cfg(test)]
mod tests;

use db::Database;
use server::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open("peachdb".to_string()).await?;
    let server = Server::new("127.0.0.1:7878", db);
    server.run().await?;
    Ok(())
}
