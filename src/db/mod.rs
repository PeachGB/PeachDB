#![allow(clippy::module_inception)]
mod codec;
mod db;
mod file;
mod wal;
pub use codec::*;
pub use db::*;

pub struct DbMeta {
    pub version: u8,
    pub record_count: u64,
    pub name: String,
}
