# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A fast, Info-ZIP compatible unzip utility written in Rust. Aims for ~5x performance improvement over Info-ZIP unzip using memory-mapped I/O and Linux kernel optimizations while maintaining CLI compatibility.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         main.rs                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ 1. Parse CLI args (args.rs)                           │   │
│  │ 2. Open ZIP file                                      │   │
│  │ 3. Decision: mmap (>1MB) vs direct I/O (<1MB)        │   │
│  │ 4. Apply Linux optimizations (madvise/fadvise)       │   │
│  │ 5. Dispatch to operation handler                     │   │
│  └──────────────────────────────────────────────────────┘   │
│                            │                                 │
│          ┌─────────────────┼─────────────────┐              │
│          │                 │                 │              │
│          ▼                 ▼                 ▼              │
│   ┌────────────┐   ┌───────────┐   ┌───────────────┐      │
│   │  list.rs   │   │extract.rs │   │test_archive.rs│      │
│   │            │   │           │   │               │      │
│   │ - Short    │   │ - Extract │   │ - CRC verify  │      │
│   │ - Verbose  │   │ - Pipe    │   │ - Error count │      │
│   │ - Comment  │   │ - Filters │   │ - Progress    │      │
│   └────────────┘   └───────────┘   └───────────────┘      │
│                            │                                 │
│          ┌─────────────────┼─────────────────┐              │
│          │                 │                 │              │
│          ▼                 ▼                 ▼              │
│   ┌────────────┐   ┌───────────┐   ┌────────────┐         │
│   │  glob.rs   │   │ utils.rs  │   │  linux.rs  │         │
│   │            │   │           │   │            │         │
│   │ - Pattern  │   │ - Format  │   │ - madvise  │         │
│   │   matching │   │ - Filter  │   │ - fallocate│         │
│   │ - *, **, ? │   │ - Convert │   │ - fadvise  │         │
│   └────────────┘   └───────────┘   └────────────┘         │
└─────────────────────────────────────────────────────────────┘

Performance optimizations:
├─ Memory-mapped I/O (>1MB files)
├─ 256KB write buffers
├─ Linux kernel hints (sequential access, pre-allocation)
├─ Buffer reuse (minimal allocations)
└─ LTO + aggressive optimization in release builds
```

### Project Structure

Non-test lines (including comprehensive documentation from Phase 11):

- `src/main.rs` - Entry point, file opening, mmap logic (87 lines)
- `src/args.rs` - CLI argument parsing (112 lines)
- `src/extract.rs` - Core extraction logic (513 lines)
- `src/list.rs` - Archive listing (167 lines)
- `src/test_archive.rs` - Integrity testing (163 lines)
- `src/glob.rs` - Pattern matching (108 lines)
- `src/utils.rs` - Shared utilities (271 lines)
- `src/linux.rs` - Linux optimizations (90 lines)
- `src/lib.rs` - Library exports (53 lines)

**Total:** 1,564 non-test lines + 860 test lines = 2,424 lines across 9 modules

## Build and Test Commands

A Makefile is available for convenience. Use `make help` to see all targets.

### Building
```bash
# Development build
make build          # or: cargo build

# Optimized release build (with LTO, codegen-units=1)
make release        # or: cargo build --release

# Install locally
make install        # or: cargo install --path .

# Clean build artifacts
make clean          # or: cargo clean
```

### Testing
```bash
# Run all tests
make test           # or: cargo test

# Run specific test
cargo test glob_match_simple_wildcard

# Run tests in specific module
cargo test glob::tests

# Run tests with output
cargo test -- --nocapture
```

### Running
```bash
# Run from source (dev build)
make run ARGS='archive.zip'         # or: cargo run -- archive.zip

# Run release build
./target/release/unzip archive.zip
```

### Code Quality

```bash
# Format code
cargo fmt
make fmt

# Run linter
cargo clippy
make lint

# Run both format and lint
make ci

# Full CI check (format + lint + test + build)
make ci-full
```

## Code Architecture

### Module Structure

The codebase is organized into focused modules in `src/`:

- **main.rs**: Entry point with memory-mapping logic
  - Opens ZIP file, decides whether to use mmap (>1MB files) or direct file I/O
  - Dispatches to appropriate command handler based on CLI args

- **args.rs**: CLI argument parsing using clap
  - Info-ZIP compatible options (-l, -v, -t, -p, -d, -o, -n, -f, -u, -j, -x, etc.)

- **extract.rs**: Core extraction logic
  - `extract_archive()`: Main extraction with progress bars, parallel extraction
  - `extract_to_pipe()`: Extract to stdout for piping
  - Uses 256KB buffer size for optimal I/O throughput

- **list.rs**: Archive listing functionality (-l and -v modes)

- **test_archive.rs**: Archive integrity testing (-t mode) with CRC verification

- **glob.rs**: Custom glob pattern matcher
  - Supports `*` (any chars except `/`), `**` (recursive), `?` (single char)
  - Core algorithm uses backtracking for wildcard matching

- **utils.rs**: Shared utilities
  - `should_extract()`: Determines if file matches include/exclude patterns
  - `format_size()`: Human-readable size formatting
  - File timestamp conversion helpers

- **linux.rs**: Linux-specific kernel optimizations (conditionally compiled)
  - `madvise_sequential()`: Hints kernel about sequential mmap access patterns
  - `fadvise_sequential()`: Hints kernel about file I/O patterns
  - `preallocate_file()`: Uses `fallocate()` to pre-allocate space and avoid fragmentation
  - `fadvise_dontneed()`: Evicts processed data from page cache
  - All functions are no-ops on non-Linux platforms

### Performance Strategy

The performance advantage comes from three key optimizations:

1. **Memory-mapped I/O (main.rs:30-42)**: Files >1MB use mmap instead of buffered reads
2. **Linux kernel hints (linux.rs)**:
   - Sequential access patterns enable aggressive read-ahead
   - Pre-allocation prevents disk fragmentation
   - Page cache management reduces memory pressure
3. **Release profile (Cargo.toml:27-32)**: LTO, opt-level=3, single codegen unit

### Cross-platform Compatibility

Linux-specific optimizations are behind `#[cfg(target_os = "linux")]` gates. The `rustix` crate is only included as a dependency for Linux targets (Cargo.toml:21-22). All linux.rs functions have no-op variants for other platforms.

### Testing

Tests are embedded in modules using `#[cfg(test)]`:
- `glob.rs`: Comprehensive glob pattern matching tests
- `extract.rs`: Extraction logic tests
- `utils.rs`: Utility function tests

Use `cargo test <module_name>::tests::<test_name>` to run specific tests.

## Release Process

Releases are automated via GitHub Actions (.github/workflows/release.yml):
- Triggered when a GitHub release is created
- Builds for multiple targets: Linux (x86_64, aarch64, musl), macOS (x86_64, aarch64), Windows (x86_64)
- Generates SHA256 checksums
- Uploads binaries to the release

To create a release:
1. Update version in Cargo.toml
2. Create a git tag: `git tag v0.x.0`
3. Push tag: `git push origin v0.x.0`
4. Create GitHub release from the tag

## Coding Standards

**For detailed coding standards, see:**
- **[.claude/rules/rust-code-style.md](.claude/rules/rust-code-style.md)** - Comprehensive code style guide including naming, formatting, error handling, performance, and documentation requirements
- **[.claude/rules/rust-testing.md](.claude/rules/rust-testing.md)** - Complete testing guidelines including coverage requirements, test organization, and best practices

### Quick Reference

**Key Requirements:**
- Maximum 500 lines per file (excluding tests)
- Minimum 60% overall test coverage (extract.rs: 80%+, glob.rs: 90%+, list.rs: 70%+)
- Never use `.unwrap()` in library code
- All public functions must have doc comments
- Use `rustfmt` and `cargo clippy` before committing
- Test naming: `test_<function>_<condition>_<expected_result>`

**Project-Specific:**
- Memory map for files >1MB (main.rs)
- 256KB buffer size (extract.rs)
- Linux optimizations behind `#[cfg(target_os = "linux")]`
- Use `anyhow::Result` for error handling
- Use `TempDir`/`TempFile` for file operations in tests

## Claude Code Workflow

### Automated Quality Checks

The project is configured with automated hooks:
- **Post-Edit Hook**: Runs `rustfmt` automatically on all .rs files
- **Session-End Hook**: Runs `cargo clippy` to check for warnings

### Proactive Agent Usage

Use specialized agents after code changes:

1. **rust-code-reviewer** - After writing/modifying code
   - Checks memory safety, error handling, performance
   - Run: Use Task tool with `subagent_type: rust-code-reviewer`

2. **rust-test-runner** - Before committing
   - Runs tests and fixes failures
   - Run: Use Task tool with `subagent_type: rust-test-runner`

3. **rust-perf-analyst** - When performance is critical
   - Profiles and suggests optimizations
   - Run: Use Task tool with `subagent_type: rust-perf-analyst`

### Pre-Commit Checklist

See **[.claude/rules/rust-code-style.md](.claude/rules/rust-code-style.md#checklist-before-committing)** for the complete checklist.

**Quick checklist:**
```bash
cargo fmt                        # Format code
cargo clippy -- -D warnings     # Check for warnings
cargo test                      # Run all tests
```

### Development Workflow Patterns

#### After Writing New Code

1. ✓ Code is written
2. → Automatic: `rustfmt` runs via post-edit hook
3. → Manual: Use `rust-code-reviewer` agent immediately
4. → Fix issues identified by reviewer
5. → Run tests: `cargo test`
6. → Fix any test failures

#### Before Committing

1. ✓ All code written and reviewed
2. → Run full test suite: `cargo test`
3. → Run clippy: `cargo clippy -- -D warnings`
4. → Use `rust-test-runner` agent if tests need fixes
5. → Verify coverage for new code
6. → Review pre-commit checklist below

#### When Performance Matters

1. → Implement feature
2. → Use `rust-code-reviewer` agent
3. → Use `rust-perf-analyst` agent for optimization suggestions
4. → Run benchmarks if available
5. → Measure before/after performance

### Agent Chaining Strategies

**For new features:**
```
rust-code-reviewer → rust-test-runner → (optional) rust-perf-analyst
```

**For bug fixes:**
```
rust-code-reviewer → rust-test-runner
```

**For refactoring:**
```
rust-code-reviewer → rust-test-runner → rust-perf-analyst
```

**For performance optimization:**
```
rust-perf-analyst → rust-code-reviewer → rust-test-runner
```

### Example Workflow Session

```
User: Add support for parallel extraction using multiple threads
Claude: [Implements parallel extraction feature]

User: Use the rust-code-reviewer agent to check the implementation

Claude: [Runs agent, identifies memory safety concerns and suggests improvements]

User: Fix the issues

Claude: [Fixes identified issues]

User: Use the rust-test-runner agent

Claude: [Runs tests, all pass with good coverage]

User: Use the rust-perf-analyst agent

Claude: [Analyzes performance, suggests thread pool optimizations]

User: Apply optimizations and commit
```

### Using Skills for Quick Commands

The project includes three skills for common tasks:

**rust-building** - Building and compiling:
```bash
# Use in Claude Code
/rust-building

# Or invoke specific commands
cargo build --release
make release
```

**rust-testing** - Running tests:
```bash
# Use in Claude Code
/rust-testing

# Or invoke specific commands
cargo test
cargo test -- --nocapture
```

**rust-benchmarking** - Performance measurement:
```bash
# Use in Claude Code
/rust-benchmarking

# Or invoke specific commands
cargo bench
```

## Security Considerations

When working with ZIP extraction:
- Validate paths to prevent directory traversal attacks
- Check for path components like `..` or absolute paths
- Verify CRC checksums for file integrity
- Handle corrupted archives gracefully
- Don't extract files outside target directory
- Be cautious with symlinks
