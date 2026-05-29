/// PeachDB basic benchmark.
///
/// Measures set, get, delete, and flush throughput directly against
/// the storage engine (no TCP overhead).
///
/// NOTE: SET and DELETE are bounded by WAL fsync-per-write (intentional
/// durability guarantee). On WSL or network filesystems, fsync is slow —
/// use a smaller N (e.g. 100) or run on a native Linux filesystem for
/// representative numbers.
///
/// Usage:
///   cargo run --release --example bench [N]
///
/// N defaults to 1_000.

use std::time::Instant;

use peachdb::db::Database;
use peachdb::dtypes::{Field, Key, Primitive};

const DEFAULT_N: usize = 1_000;
const FLUSH_N: usize = 5;
const DB_NAME: &str = "peachdb_bench_tmp";

#[tokio::main]
async fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_N);

    println!("PeachDB benchmark  —  N = {}", n);
    println!("{:-<50}", "");

    cleanup().await;
    let db = Database::open(DB_NAME.to_string()).await.expect("open db");

    // Pre-build keys and fields to exclude allocation from timing
    let keys: Vec<Key> = (0..n).map(|i| Key::from(format!("key_{:08}", i).as_str())).collect();
    let field = Field::Primitive(Primitive::String("benchmark_value".to_string()));

    // ── SET ──────────────────────────────────────────────────────────────────
    let t = Instant::now();
    for key in &keys {
        db.set(key.clone(), field.clone()).await.expect("set");
    }
    print_result("SET", n, t.elapsed().as_millis());

    // ── GET ──────────────────────────────────────────────────────────────────
    let t = Instant::now();
    for key in &keys {
        db.get(key).await.expect("get");
    }
    print_result_us("GET", n, t.elapsed().as_micros());

    // ── FLUSH (batched) ──────────────────────────────────────────────────────
    // flush commits all WAL entries written above in one shot, then we re-insert
    // smaller batches to measure repeated flushes
    db.flush().await.expect("initial flush");

    let batch = (n / FLUSH_N).max(1);
    let flush_keys: Vec<Key> = (0..batch)
        .map(|i| Key::from(format!("flush_key_{:08}", i).as_str()))
        .collect();

    let t = Instant::now();
    for _ in 0..FLUSH_N {
        for key in &flush_keys {
            db.set(key.clone(), field.clone()).await.expect("set");
        }
        db.flush().await.expect("flush");
    }
    let flush_ms = t.elapsed().as_millis();
    println!(
        "{:<8}  x{:<6}  {:>6} ms   ({} ms/flush)",
        "FLUSH",
        FLUSH_N,
        flush_ms,
        flush_ms / FLUSH_N as u128,
    );

    // ── DELETE ───────────────────────────────────────────────────────────────
    // re-insert the original keys (flushed above), then delete them
    for key in &keys {
        db.set(key.clone(), field.clone()).await.expect("set");
    }
    db.flush().await.expect("flush before delete bench");

    let t = Instant::now();
    for key in &keys {
        db.delete(key.clone()).await.expect("delete");
    }
    print_result("DELETE", n, t.elapsed().as_millis());

    drop(db);
    cleanup().await;
}

fn print_result(label: &str, n: usize, ms: u128) {
    let ops_sec = if ms == 0 { 0 } else { n as u128 * 1000 / ms };
    println!(
        "{:<8}  x{:<6}  {:>6} ms   ({} ops/sec)",
        label, n, ms, ops_sec
    );
}

fn print_result_us(label: &str, n: usize, us: u128) {
    let ops_sec = if us == 0 { 0 } else { n as u128 * 1_000_000 / us };
    println!(
        "{:<8}  x{:<6}  {:>6} µs   ({} ops/sec)",
        label, n, us, ops_sec
    );
}

async fn cleanup() {
    let _ = tokio::fs::remove_file(format!("{}.db", DB_NAME)).await;
    let _ = tokio::fs::remove_file(format!("{}.dbidx", DB_NAME)).await;
    let _ = tokio::fs::remove_file(format!("{}.wal", DB_NAME)).await;
}
