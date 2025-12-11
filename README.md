# unzip

A fast, reliable unzip utility written in Rust. Info-ZIP compatible with modern enhancements.

## Features

- **Fast**: Memory-mapped I/O for large files, optimized release builds with LTO
- **Reliable**: Memory-safe Rust, comprehensive CRC verification, extensive test coverage
- **Compatible**: Drop-in replacement for Info-ZIP unzip with matching CLI options
- **Modern**: Progress bars, glob patterns (`**/*.rs`), colored output
- **Full-featured**: All major Info-ZIP options supported

### Supported Operations

- Extract archives with progress visualization
- List contents (short and verbose formats)
- Test archive integrity with CRC verification
- Extract to stdout/pipe
- Selective extraction with glob patterns
- Exclude files with patterns
- Freshen/update modes
- Preserve Unix permissions and timestamps

### Compression Support

- Deflate, Deflate64
- LZMA, LZMA2
- Bzip2
- Zstd
- AES encrypted archives

## Installation

### From source

```bash
git clone https://github.com/yourusername/unzip.git
cd unzip
cargo build --release
```

The binary will be at `target/release/unzip`.

### Using Cargo

```bash
cargo install --path .
```

## Usage

```
unzip [OPTIONS] <FILE> [PATTERN]...
```

### Arguments

| Argument | Description |
|----------|-------------|
| `<FILE>` | Path to the ZIP file to extract |
| `[PATTERN]...` | Files to extract (supports glob patterns) |

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--directory <DIR>` | `-d` | Extract files to specified directory |
| `--list` | `-l` | List contents (short format) |
| `--verbose` | `-v` | List contents (verbose format with compression ratio) |
| `--test` | `-t` | Test archive integrity |
| `--pipe` | `-p` | Extract to stdout (for piping) |
| `--comment` | `-z` | Display archive comment only |
| `--overwrite` | `-o` | Overwrite existing files without prompting |
| `--never-overwrite` | `-n` | Never overwrite existing files |
| `--freshen` | `-f` | Only update existing files (don't create new) |
| `--update` | `-u` | Update files (freshen + create new if needed) |
| `--junk-paths` | `-j` | Extract without directory structure |
| `--case-insensitive` | `-C` | Match filenames case-insensitively |
| `--lowercase` | `-L` | Convert filenames to lowercase |
| `--quiet` | `-q` | Quiet mode (-q less output, -qq minimal) |
| `--threads <NUM>` | `-T` | Number of threads (default: auto) |
| `--exclude <PATTERN>` | `-x` | Exclude files matching pattern |
| `--help` | `-h` | Print help |
| `--version` | `-V` | Print version |

## Examples

### Basic Operations

```bash
# Extract all files to current directory
unzip archive.zip

# Extract to a specific directory
unzip -d /tmp/output archive.zip

# List archive contents
unzip -l archive.zip

# List with compression details
unzip -v archive.zip

# Test archive integrity
unzip -t archive.zip
```

### Selective Extraction

```bash
# Extract only .txt files
unzip archive.zip '*.txt'

# Extract all Rust source files
unzip archive.zip '**/*.rs'

# Extract everything except logs
unzip archive.zip -x '*.log' -x '*.tmp'

# Extract specific file to stdout
unzip -p archive.zip config.json | jq .
```

### Overwrite Control

```bash
# Overwrite all existing files
unzip -o archive.zip

# Never overwrite (skip existing)
unzip -n archive.zip

# Only update files that are older
unzip -u archive.zip

# Only freshen existing files (don't create new)
unzip -f archive.zip
```

### Other Options

```bash
# Extract without directory structure (flatten)
unzip -j archive.zip

# Case-insensitive pattern matching
unzip -C archive.zip '*.TXT'

# Convert filenames to lowercase
unzip -L archive.zip

# Quiet extraction
unzip -q archive.zip

# Very quiet (errors only)
unzip -qq archive.zip
```

## Glob Pattern Syntax

| Pattern | Description |
|---------|-------------|
| `*` | Matches any characters except `/` |
| `**` | Matches any characters including `/` (recursive) |
| `?` | Matches any single character |

### Pattern Examples

- `*.txt` - All `.txt` files in root
- `src/*.rs` - All `.rs` files directly in `src/`
- `**/*.rs` - All `.rs` files in any directory
- `src/**` - Everything under `src/`
- `file?.txt` - `file1.txt`, `fileA.txt`, etc.

## Performance

This implementation is optimized for speed:

- **Memory-mapped I/O**: Files >1MB use mmap for faster reading
- **Buffered writing**: 256KB write buffers for efficient disk I/O
- **LTO builds**: Link-time optimization in release builds
- **Minimal allocations**: Reuses buffers where possible
- **Linux kernel optimizations** (on Linux):
  - `madvise(MADV_SEQUENTIAL)` - Hint for sequential access patterns
  - `madvise(MADV_WILLNEED)` - Pre-fault pages for faster access
  - `fallocate()` - Pre-allocate disk space to avoid fragmentation
  - `fadvise(POSIX_FADV_SEQUENTIAL)` - Hint for file access patterns

### Benchmark Results

Tested on Linux 6.18 with a 1GB ZIP archive containing 100 files (10MB each, stored/uncompressed):

| Tool | Time | Speedup |
|------|------|---------|
| Info-ZIP unzip 6.00 | 4.92s | 1x (baseline) |
| **This (Rust unzip)** | **0.95s** | **~5x faster** |

```bash
# Reproduce the benchmark
$ time /usr/bin/unzip -q -o -d out_system test.zip
4.92s

$ time ./target/release/unzip -q -o -d out_rust test.zip
0.95s
```

## Comparison with Info-ZIP

| Feature | Info-ZIP | This |
|---------|----------|------|
| Memory safety | C (manual) | Rust (guaranteed) |
| Progress bar | No | Yes |
| Glob patterns | Basic | Extended (`**`) |
| Memory-mapped I/O | No | Yes (>1MB files) |
| Last updated | 2009 | Active |
| **Performance** | Baseline | **~5x faster** |

### Compatibility

This tool aims for CLI compatibility with Info-ZIP unzip. Most common options work identically:

```bash
# These work the same way
unzip -l archive.zip
unzip -d /tmp archive.zip
unzip -o archive.zip
unzip archive.zip '*.txt' -x '*.log'
```

## Dependencies

- [zip](https://crates.io/crates/zip) - ZIP archive handling
- [clap](https://crates.io/crates/clap) - CLI argument parsing
- [anyhow](https://crates.io/crates/anyhow) - Error handling
- [indicatif](https://crates.io/crates/indicatif) - Progress bars
- [memmap2](https://crates.io/crates/memmap2) - Memory-mapped files
- [filetime](https://crates.io/crates/filetime) - File timestamp handling
- [crc32fast](https://crates.io/crates/crc32fast) - Fast CRC verification
- [rustix](https://crates.io/crates/rustix) - Linux syscalls for kernel optimizations (Linux only)

## License

MIT
