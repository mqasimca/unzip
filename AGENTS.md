# AGENTS.md

Project: `unzip` — a fast, Info-ZIP compatible `unzip` utility written in Rust.

This file provides **high-level architectural and behavioral context** for Codex
and other automated agents. It describes *what the project is*, *how it is
structured*, and *which invariants must not be violated*.

---

## Project Overview

- Language: Rust (stable)
- Type: Single crate producing a binary and a reusable library
- Goal: Be compatible with Info-ZIP `unzip` in flags, behavior, and output
- Focus areas:
  - Correctness and compatibility
  - Performance on large archives
  - Safety when handling malformed or malicious ZIP files

---

## High-Level Architecture

- One crate with:
  - Binary entrypoint (`src/main.rs`)
  - Library interface (`src/lib.rs`)
- CLI is Info-ZIP compatible and implemented using `clap`
- Core operations:
  - Extract files
  - List archive contents
  - Test archive integrity (`-t`)
  - Zipinfo-style listing (`-Z`)
  - Pipe file contents to stdout

---

## Code Structure

- `src/main.rs`  
  CLI entrypoint. Parses arguments, selects I/O strategy (mmap vs std I/O),
  dispatches to list / test / extract / pipe / zipinfo modes.

- `src/lib.rs`  
  Library exports and module wiring.

- `src/args.rs`  
  CLI definitions using `clap`. Flags and semantics are intended to match
  Info-ZIP `unzip`.

- `src/extract.rs`  
  Core extraction logic:
  - Overwrite and conflict handling
  - Progress reporting
  - Timestamp and permission restoration
  - Piping file contents to stdout
  - Safety checks on paths and metadata

- `src/list.rs`  
  Implements `-l`, `-v`, and `-z` listing modes, including archive comments.

- `src/test_archive.rs`  
  Integrity testing (`-t`), including CRC32 validation.

- `src/zipinfo.rs`  
  Implements Info-ZIP compatible `zipinfo` output (`-Z` modes).

- `src/glob.rs`  
  Glob matcher used for include/exclude filtering.

- `src/utils.rs`  
  Shared helpers:
  - Pattern filtering
  - Size/date formatting
  - Time and permission conversions

- `src/password.rs`  
  Password handling and interactive prompt for encrypted archives.

- `src/linux.rs`  
  Linux-specific performance helpers:
  - `madvise`
  - `fadvise`
  - `fallocate`
  Guarded by `cfg(target_os = "linux")` and no-ops elsewhere.

- `benches/extraction_bench.rs`  
  Criterion benchmarks for extraction and glob matching.

---

## Performance Notes

- Files larger than 1MB use memory-mapped I/O (mmap)
- Smaller files use standard buffered I/O
- Default buffer size is 256KB
- Linux-specific kernel hints are applied where available
- Performance-sensitive paths must avoid unnecessary allocations and syscalls

---

## Build & Run

- Debug build:  
  `cargo build`

- Release build:  
  `cargo build --release`

- Run locally:  
  `cargo run -- <args>`

- Install from source:  
  `cargo install --path .`

- Makefile shortcuts:
  - `make build`
  - `make release`
  - `make run ARGS='archive.zip'`

---

## Tests

- Unit tests live alongside code using `#[cfg(test)]`
- Run all tests:  
  `cargo test`
- Makefile target:  
  `make test`

---

## Benchmarks

- Criterion configured in `Cargo.toml`
- Bench file: `benches/extraction_bench.rs`
- Run benchmarks:  
  `cargo bench`

Note: the current Makefile `bench` target does **not** invoke `cargo bench`.

---

## Critical Invariants (Must Not Change)

- CLI flags, defaults, and semantics must remain Info-ZIP compatible
- Output format should remain byte-for-byte compatible when possible
- Extraction must be safe against path traversal and malformed metadata
- Linux-specific optimizations must remain guarded by `cfg(target_os = "linux")`
- Pattern filtering logic is centralized in `utils::should_extract`

