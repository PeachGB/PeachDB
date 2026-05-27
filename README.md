# PeachDB

PeachDB is a work-in-progress key/value database prototype written in Rust (Tokio).
This README reflects the current development state — the project is experimental.

Goals
- Small, embeddable file-backed KV store
- Simple on-disk layout with a separate index file
- Fast in-memory field cache and async API surface

What's implemented 
- Core storage with data file and persisted index (index file is truncated and rewritten on update).
- In-memory Fields map protected by tokio::Mutex.
- Codec refactor: serialization moved into `src/db/codec.rs`; Fields now include the Dtype as the first byte of their payload.
- Unified error enum at `src/error.rs` (thiserror-based).
- Thin public wrapper at `src/interface.rs` (open/get/insert/persist/flush/close).
- Basic WAL encoder/decoder skeleton and some WAL helpers (partial).

What works today
- Create/open database files
- Insert into in-memory cache and persist entries to disk (append + index update)
- Load index and reload records into memory
- Encode/decode primitives and fields (round-trip in-progress)

Known limitations / TODO
- Not crash-safe: no atomic index swap (temp file + rename) or reliable fsyncs yet.
- WAL replay is incomplete; replay and commit semantics need work.
- Some modules are still missing or inconsistent (server/protocol, legacy type representations).
- Several unwrap/expect usages remain; many call sites need proper PeachDbError conversion.
- No unit tests for codec or persistence yet.

Next steps (recommended)
- Harmonize field/primitive types across the crate (remove legacy char-array code).
- Add unit tests for codec round-trips and WAL replay.
- Implement atomic index replacement and durable flush (fsync) for safety.
- Finish WAL replay and integrate with startup recovery.
- Clean up module layout and fix remaining compiler warnings/errors.

Using the prototype
- cargo check (may fail until remaining modules are completed)
- Use `src/interface.rs::Database` as the public entry point (open/insert/persist/get/flush).

Contributing
All contributions welcome — open issues or send PRs. This repository is evolving rapidly; breaking changes are expected.
