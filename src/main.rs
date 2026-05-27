mod db;
mod dtypes;
mod error;
mod server;
#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    Ok(())
}
