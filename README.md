# PeachDB

PeachDB is a work-in-progress key/value database prototype written in Rust with Tokio.

## Goals
- Small, embeddable file-backed KV store
- Separate on-disk data, index, and WAL files
- Async API with a simple public surface

## Current state
- Core storage is implemented.
- `FileHandler` wraps file I/O for `.db`, `.dbidx`, and `.wal`.
- Codec lives in `src/db/codec.rs`.
- `Field` payloads now start with the `Dtype` byte.
- Unified error type lives in `src/error.rs`.
- Thin public wrapper lives in `src/interface.rs`.
- Unit tests exist under `src/tests/`.

## What works
- Create/open database files
- Insert, get, delete, flush, and reopen data
- Persist WAL entries and rebuild the index
- Encode/decode primitives, fields, WAL entries, and index entries
- Run the current test suite successfully

## Known limitations
- Not crash-safe yet; durability can still be improved.
- WAL recovery semantics still need hardening.
- Some helpers and imports are still unused.
- The public API is still experimental and may change.

## Using it
- `cargo test`
- `src/interface.rs::Database` is the main entry point

## Contributing
Breaking changes are expected while the project evolves.
