# PeachDB

PeachDB is an async, file-backed key/value database written in Rust with Tokio. It exposes a binary TCP protocol and stores data across three on-disk files per database. The storage engine, protocol, and TCP server are fully functional.

---

## Table of contents

- [Architecture](#architecture)
- [Data flow](#data-flow)
- [File formats](#file-formats)
- [Type system](#type-system)
- [TCP protocol](#tcp-protocol)
- [Rust API](#rust-api)
- [Build and run](#build-and-run)
- [Examples](#examples)
- [Test suite](#test-suite)
- [Known limitations](#known-limitations)

---

## Architecture

```
src/
├── lib.rs                — public crate root (re-exports db, dtypes, error, server)
├── main.rs               — entry point: opens DB, starts TCP server
├── dtypes.rs             — Key, Field, Dtype, Primitive, serialisation helpers
├── error.rs              — unified PeachDbError (thiserror)
├── db/
│   ├── mod.rs            — re-exports
│   ├── db.rs             — Database struct: open, get, set, delete, flush, keys
│   ├── codec.rs          — binary encoder/decoder structs for .db, .dbidx, .wal
│   ├── file.rs           — FileHandler: single-task mpsc wrapper around tokio::fs::File
│   └── wal.rs            — WAL: append_entry, replay, checkpoint
└── server/
    ├── mod.rs            — re-exports
    ├── protocol.rs       — ProtocolEncoder, ProtocolDecoder, Request, Response
    └── server.rs         — Server: TCP accept loop, handle_connection, handle_request
```

### Module responsibilities

| Module | Responsibility |
|---|---|
| `dtypes` | All value types and their serialisation (`bytes_from_field`, `field_from_bytes`, etc.) |
| `db::codec` | Stateful encoder/decoder structs for `.db`, `.dbidx`, and `.wal` binary formats |
| `db::file` | Serialises all I/O to a single Tokio task per file, eliminating concurrent-seek races |
| `db::wal` | Append-only log; replayed on open; checkpointed (truncated) after each flush |
| `db::db` | Public database API; owns in-memory state (`fields` + `index` maps) behind an `RwLock` |
| `server::protocol` | Stateless encoder/decoder for the TCP wire format |
| `server::server` | Accept loop; spawns one task per connection; routes requests to `Database` |

---

## Data flow

### Write path

```
db.set(key, field)
  └─ WAL::append_entry(Set)  →  append to .wal on disk  →  fsync
  └─ state.fields.insert(key, field)  →  in-memory, immediately visible to reads
```

### Flush

```
db.flush()
  └─ WAL::pop_uncommitted_entries()
  └─ for each Set    →  encode record  →  FileHandler::append to .db
  └─ for each Delete →  write tombstone (zero-filled) over existing record in .db
  └─ file.sync_all()
  └─ update record_count in .db header
  └─ rebuild .dbidx from state.index
  └─ WAL::checkpoint()  →  truncate .wal to 0 bytes
```

### Read path

```
db.get(key)  →  state.fields.get(key)   (no disk I/O)
```

### Open / reopen

```
Database::open(name)
  ├─ if .db is empty  →  Database::new  →  write header, empty state
  └─ otherwise        →  Database::init
       ├─ read .db into memory  →  DBDecoder::decode_db_file → (fields, meta)
       ├─ read .dbidx (or rebuild from .db if empty)  →  index map
       └─ WAL::replay  →  re-apply any uncommitted entries
```

---

## File formats

All multi-byte integers are **little-endian** unless stated otherwise. The TCP protocol uses big-endian.

### `.db` — data file

#### Header (114 bytes, at offset 0)

| Field | Size | Value |
|---|---|---|
| Key `Header--` | 8 B | ASCII literal |
| Magic | 8 B | `0x50656163684442` (u64 LE) |
| Key `Version-` | 8 B | ASCII literal |
| Version major | 1 B | `0x01` |
| Version minor | 1 B | `0x00` |
| Key `RecCount` | 8 B | ASCII literal |
| Record count | 8 B | u64 LE — updated on every flush |
| Key `DB Name-` | 8 B | ASCII literal |
| DB name | 64 B | UTF-8, null-padded |

#### Live record

```
[REC\x01 : 4 B]  [key_len : u16 LE]  [field_len : u32 LE]  [crc32 : u32 LE]
[key     : key_len B]
[field   : field_len B]   ← dtype byte followed by payload (see Type system)
```

CRC-32 (ISO HDLC) is computed over `[key_len][field_len][0x00000000][key][field]` — the CRC field itself is zeroed before hashing.

#### Deleted record (tombstone)

Same layout as a live record, magic replaced by `[0x00 0x00 0x00 0x00]`, all remaining bytes zeroed. Total size is identical to the original record, so offsets of later records are stable.

---

### `.dbidx` — index file

No header. A flat sequence of entries, one per live key:

```
[key_len : u16 LE]  [key : key_len B]  [field_len : u32 LE]  [offset : u64 LE]
```

`offset` is the byte position of the record's magic number in `.db`. Rebuilt from scratch on every flush; if absent at open time, rebuilt from `.db`.

---

### `.wal` — write-ahead log

Append-only. Entries are written on every `set` / `delete` before the in-memory state is updated. The file is truncated to zero after a successful flush (checkpoint). Each write is followed by an `fsync`.

| Entry | Encoding |
|---|---|
| Set | `[0x01][key_len : u16 LE][key bytes][field_len : u32 LE][field bytes]` |
| Delete | `[0x02][key_len : u16 LE][key bytes]` |

`field bytes` includes the dtype byte as its first byte (same encoding as in `.db`).

---

## Type system

Every stored value is a `Field`, serialised as `[dtype : 1 B][payload]`:

| Dtype | Byte | Rust type | Payload |
|---|---|---|---|
| Raw | `0x00` | `Arc<[u8]>` | raw bytes (length from `field_len`) |
| Integer | `0x01` | `i64` | 8 B LE |
| Float | `0x02` | `f64` | 8 B LE |
| String | `0x03` | `String` | `[len : u32 LE][utf-8 bytes]` |
| Boolean | `0x04` | `bool` | `0x00` or `0x01` (1 B) |
| Array(T) | `0x10 \| T` | `Arc<[Primitive]>` | `[count : u32 LE][element …]` |

Arrays can hold any primitive type. The inner type byte is the low nibble of the dtype byte (`dtype & 0x0F`).

---

## TCP protocol

All multi-byte integers in the protocol are **big-endian**.

### Framing

Every message (request or response) is length-prefixed:

```
[msg_len : u32 BE]  [message : msg_len B]
```

### Request

```
[version : 1 B = 0x01]  [cmd : 1 B]  [key_len : u16 BE]  [key : key_len B]
                                       └── omitted for Ping and Keys ──┘
// SET only, after key:
[val_len : u32 BE]  [val : val_len B]   ← dtype byte + payload
```

| Command | Byte |
|---|---|
| Ping | `0x01` |
| Get | `0x02` |
| Set | `0x03` |
| Del | `0x04` |
| Keys | `0x05` |

### Response

```
[status : 1 B]  [payload_len : u32 BE]  [payload : payload_len B]
```

| Status | Byte | Payload |
|---|---|---|
| Ok | `0x00` | field bytes if value present, empty otherwise |
| NotFound | `0x01` | empty |
| TypeError | `0x02` | empty |
| ServerError | `0x03` | UTF-8 error message |
| InvalidRequest | `0x04` | empty |
| Keys | `0x05` | `[key_len : u16 BE][key bytes]` repeated for each key |

### Connection semantics

- The server reads requests in a loop on each connection.
- On `UnexpectedEof` or `ConnectionReset` the connection is closed cleanly.
- An invalid request (bad version byte, unknown command) returns `InvalidRequest` and the connection remains open — the next request is read normally.
- `Del` on a non-existent key returns `NotFound`.

---

## Rust API

```rust
// Open or create a database (creates name.db, name.dbidx, name.wal)
let db = Database::open("mydb".to_string()).await?;

// Write
db.set(Key::from("hello"), Field::Primitive(Primitive::String("world".into()))).await?;

// Read (in-memory, no disk I/O)
let field = db.get(&Key::from("hello")).await?;

// Delete
db.delete(Key::from("hello")).await?;

// List all live keys
let keys: Vec<Key> = db.keys().await?;

// Persist WAL entries to .db, rebuild .dbidx, checkpoint WAL
db.flush().await?;
```

```rust
// Start the TCP server
let db = Database::open("peachdb".to_string()).await?;
let server = Server::new("127.0.0.1:7878", db);
server.run().await?;   // loops forever, spawns a task per connection
```

---

## Build and run

**Requirements:** Rust stable, Tokio (managed via Cargo).

```bash
cargo build          # compile
cargo run            # start server on 127.0.0.1:7878
cargo test           # run all 39 tests
cargo test <name>    # run a single test by name (substring match)
cargo clippy         # lint
```

The server creates `peachdb.db`, `peachdb.dbidx`, and `peachdb.wal` in the working directory on first run.

---

## Examples

### CLI client

A synchronous command-line client that connects to a running server:

```bash
cargo run --example peachdb-cli -- ping
cargo run --example peachdb-cli -- set name Alice
cargo run --example peachdb-cli -- get name
cargo run --example peachdb-cli -- keys
cargo run --example peachdb-cli -- del name

# custom server address
cargo run --example peachdb-cli -- --addr 10.0.0.1:7878 get name
```

Values passed to `set` are auto-typed: `true`/`false` → Boolean, integers → Integer, floats → Float, anything else → String.

### Benchmark

Measures set, get, delete, and flush throughput directly against the storage engine (no TCP overhead):

```bash
cargo run --release --example bench         # default N=1000
cargo run --release --example bench -- 100  # custom N
```

SET and DELETE are bounded by WAL fsync-per-write. On WSL or network filesystems, fsync is slow — use a smaller N or run on a native Linux filesystem for representative numbers.

---

## Test suite

39 tests across four files:

| File | What it covers |
|---|---|
| `tests/codec_tests.rs` | Primitive and field roundtrips, record encode/decode |
| `tests/db_tests.rs` | Set/get/flush/reopen persistence, delete and flush, tombstone correctness after reopen |
| `tests/protocol_tests.rs` | Request and response roundtrips for all variants, error cases |
| `tests/server_tests.rs` | `handle_request` unit tests for all commands; TCP integration tests via real connections |

Each database test creates uniquely named files and cleans them up via a `DbCleanup` RAII guard — files are removed even if the test panics.

---

## Known limitations

- **WAL is not crash-safe after checkpoint.** If the process dies between `sync_all()` on `.db` and `WAL::checkpoint()`, the WAL still holds the committed entries and replay is safe. However, there is no crash recovery path for a partially written record within a flush.
- **No compaction.** Tombstones accumulate in `.db`; the file never shrinks.
- **Server binds on a fixed address** (`127.0.0.1:7878` hardcoded in `main.rs`); no config file or CLI flags yet.
- **No authentication or access control** on the TCP server.
- Several unused `pub` items and imports produce compiler warnings (`cargo clippy` lists them).
