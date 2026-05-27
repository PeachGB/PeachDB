mod codec;
mod db;
mod wal;
pub use db::*;

pub struct DbMeta {
    pub version: u8,
    pub record_count: u64,
    pub name: String,
}
