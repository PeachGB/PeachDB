use std::sync::atomic::{AtomicU64, Ordering};

static TEST_ID: AtomicU64 = AtomicU64::new(0);

pub fn unique_db_name(prefix: &str) -> String {
    format!("{}_{}", prefix, TEST_ID.fetch_add(1, Ordering::Relaxed))
}

pub struct DbCleanup {
    pub name: String,
}

impl Drop for DbCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(format!("{}.db", self.name));
        let _ = std::fs::remove_file(format!("{}.dbidx", self.name));
        let _ = std::fs::remove_file(format!("{}.wal", self.name));
    }
}
