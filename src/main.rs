use peachdb::db::Database;
use peachdb::server::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open("peachdb".to_string()).await?;
    let server = Server::new("127.0.0.1:7878", db);
    server.run().await?;
    Ok(())
}
