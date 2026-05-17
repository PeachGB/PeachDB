# PeachDB

PeachDB is a work-in-progress database prototype written in Rust with Tokio.

It currently explores:
- file-backed storage
- an in-memory field cache
- a persisted index file
- loading records back from disk
- a small public wrapper API on top of the storage layer

## Current status

This project is **not production-ready** yet. The codebase is still in an early prototyping phase and the storage format is still evolving.

## Project structure

- `src/db.rs`  
  Core storage engine, record encoding, index handling, loading, and persistence.

- `src/interface.rs`  
  Public-facing wrapper around the database backend.

- `src/main.rs`  
  Temporary entry point used while the prototype is being shaped.

## What works today

- opening or creating the database files
- keeping records in memory
- persisting records to disk
- maintaining an index file
- reloading persisted records

## Known limitations

- the record format is still experimental
- complex field types are not fully supported on load yet
- error handling is still rough in places
- the network/server entry point is only a placeholder
- APIs may change without notice while the prototype evolves

## Planned direction

- stabilize the file format
- improve error handling
- finish the public API
- add a real entry point or server layer
- document supported field types and record lifecycle

## Notes

This repository is intentionally changing fast. Expect internal APIs and on-disk format details to shift while the prototype is being refined.
