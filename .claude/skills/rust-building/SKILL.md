---
name: rust-building
description: Build and compile Rust projects with cargo. Use for building, checking, or troubleshooting compile errors.
allowed-tools: Bash(cargo:*), Bash(make:*), Read, Grep, Glob
---

# Rust Building Skill

Quick access to Rust build commands and compile error diagnosis for the unzip project.

## Quick Commands

### Basic Building

```bash
# Development build (fast compile, no optimizations)
cargo build
make build

# Release build (optimized, slower compile)
cargo build --release
make release

# Check for errors without building (fastest)
cargo check

# Clean build artifacts
cargo clean
make clean
```

### Advanced Building

```bash
# Build with all features
cargo build --all-features

# Build for specific target
cargo build --target x86_64-unknown-linux-musl

# Build with verbose output
cargo build --verbose

# Build and show warnings
cargo build 2>&1 | grep warning
```

## Compile Error Diagnosis

### Step 1: Read the Error Message

Rust's compiler errors are detailed and helpful:
- **Error code**: E0XXX (can be explained with `rustc --explain E0XXX`)
- **Location**: File path and line number
- **Context**: Shows relevant code
- **Suggestion**: Often includes fix

### Step 2: Common Error Types

#### Borrow Checker Errors

```
error[E0502]: cannot borrow `x` as mutable because it is also borrowed as immutable
```

**Fix**: Restructure code to avoid overlapping borrows
```rust
// Before (error)
let r = &x;
let m = &mut x;  // Error!

// After (fixed)
let r = &x;
drop(r);  // Or let r go out of scope
let m = &mut x;  // OK
```

#### Lifetime Errors

```
error[E0106]: missing lifetime specifier
```

**Fix**: Add explicit lifetime annotations
```rust
// Before
fn longest(x: &str, y: &str) -> &str {  // Error!

// After
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {  // OK
```

#### Type Errors

```
error[E0308]: mismatched types
```

**Fix**: Verify types match or add conversion
```rust
// Before
let x: u32 = 10;
let y: u64 = x;  // Error!

// After
let x: u32 = 10;
let y: u64 = x as u64;  // OK - explicit conversion
// Or
let y: u64 = x.into();  // OK - using Into trait
```

#### Missing Trait Implementations

```
error[E0277]: the trait bound `Foo: Clone` is not satisfied
```

**Fix**: Add derive or implement trait
```rust
#[derive(Clone)]  // Add this
struct Foo {
    data: String,
}
```

### Step 3: Use Compiler Suggestions

Rust compiler often suggests fixes:
```
help: consider borrowing here
  |
5 |     function(&value)
  |              ^
```

Apply the suggestion and rebuild.

### Step 4: Explain Error Codes

```bash
# Get detailed explanation
rustc --explain E0502

# Example explanations for common errors:
# E0502 - Cannot borrow as mutable while immutably borrowed
# E0308 - Mismatched types
# E0106 - Missing lifetime specifier
# E0277 - Trait bound not satisfied
# E0382 - Use of moved value
```

## Release Build Optimization

The unzip project uses aggressive release optimizations in `Cargo.toml`:

```toml
[profile.release]
opt-level = 3           # Maximum optimization
lto = true              # Link-time optimization (slower build, faster binary)
codegen-units = 1       # Single codegen unit (better optimization)
panic = "abort"         # Smaller binary, no unwinding
strip = true            # Remove debug symbols
```

### When to Use Each Profile

**Debug build (`cargo build`)**:
- Fast compilation (~5-10 seconds)
- Large binary (~5-10MB)
- Includes debug symbols
- No optimizations
- **Use for**: Development, debugging

**Release build (`cargo build --release`)**:
- Slow compilation (~30-60 seconds)
- Small binary (~1-2MB stripped)
- No debug symbols
- Maximum optimizations
- **Use for**: Production, benchmarking, distribution

## Build Troubleshooting

### Problem: Slow Compile Times

**Solutions:**
```bash
# Use sccache for caching
cargo install sccache
export RUSTC_WRAPPER=sccache

# Enable incremental compilation (default in debug)
export CARGO_INCREMENTAL=1

# Build only what changed
cargo build

# Use cargo-watch for auto-rebuild
cargo install cargo-watch
cargo watch -x build
```

### Problem: Out of Memory During Build

**Solutions:**
```bash
# Reduce parallel jobs
cargo build -j 2

# Build with less optimization temporarily
cargo build --release --profile dev
```

### Problem: Linker Errors

```
error: linking with `cc` failed
```

**Solutions:**
```bash
# Install missing system libraries
sudo apt-get install build-essential

# For musl target
sudo apt-get install musl-tools

# For aarch64 cross-compilation
sudo apt-get install gcc-aarch64-linux-gnu
```

### Problem: Cannot Find Crate

```
error[E0432]: unresolved import `some_crate`
```

**Solutions:**
```bash
# Add dependency to Cargo.toml
[dependencies]
some_crate = "1.0"

# Update dependencies
cargo update

# Check Cargo.lock exists
ls Cargo.lock
```

## Platform-Specific Builds

### Linux (current platform)

```bash
# Native build
cargo build --release

# Musl (static binary)
cargo build --release --target x86_64-unknown-linux-musl

# Cross-compile for ARM
cargo build --release --target aarch64-unknown-linux-gnu
```

### macOS

```bash
# x86_64
cargo build --release --target x86_64-apple-darwin

# Apple Silicon
cargo build --release --target aarch64-apple-darwin
```

### Windows

```bash
cargo build --release --target x86_64-pc-windows-msvc
```

## Build Workflow Best Practices

### Development Cycle

```bash
# 1. Check for errors (fastest)
cargo check

# 2. Run tests
cargo test

# 3. Build if needed
cargo build

# 4. Run program
cargo run -- archive.zip
```

### Pre-Commit Checklist

```bash
# 1. Format code
cargo fmt

# 2. Check for errors
cargo check

# 3. Run linter
cargo clippy

# 4. Run tests
cargo test

# 5. Build release
cargo build --release

# 6. Verify binary works
./target/release/unzip --version
```

### CI/CD Build Commands

```bash
# GitHub Actions / CI build
cargo build --release --locked --all-features
cargo test --release --locked --all-features
cargo clippy -- -D warnings
```

## Monitoring Build Output

### Check Binary Size

```bash
# After release build
ls -lh target/release/unzip

# Compare sizes
du -h target/debug/unzip target/release/unzip

# Check stripped vs unstripped
strip target/release/unzip
ls -lh target/release/unzip
```

### Verify Optimizations

```bash
# Check if LTO was applied
cargo rustc --release -- --print native-static-libs

# Verify strip worked (should show minimal symbols)
nm -D target/release/unzip | wc -l
```

### Build Time Analysis

```bash
# Time the build
time cargo build --release

# Detailed timing
cargo build --release --timings
# Opens flamegraph in browser

# Per-crate timing
cargo build --release -vv 2>&1 | grep "Compiling"
```

## Makefile Integration

The unzip project has a Makefile for convenience:

```bash
# Show available targets
make help

# Build debug
make build

# Build release
make release

# Run with args
make run ARGS='-l archive.zip'

# Clean
make clean

# Install
make install
```

## Quick Reference

| Command | Purpose | Speed | Use Case |
|---------|---------|-------|----------|
| `cargo check` | Check errors | ⚡⚡⚡ | During development |
| `cargo build` | Debug build | ⚡⚡ | Testing locally |
| `cargo build --release` | Release build | ⚡ | Production/benchmarks |
| `cargo clean` | Remove artifacts | N/A | Fresh build |
| `make build` | Debug via Make | ⚡⚡ | Convenience |
| `make release` | Release via Make | ⚡ | Convenience |

## Common Build Errors for Unzip Project

### rustix Errors (Linux Optimizations)

If building on non-Linux:
```
error: failed to resolve: could not find `mm` in `rustix`
```

**Fix**: This is expected - rustix is Linux-only. Build on Linux or disable Linux features.

### ZIP Crate Version Issues

```
error: package `zip v2.2.0` cannot be built
```

**Fix**: Update zip crate
```bash
cargo update -p zip
```

### Memory Map Errors

```
error: failed to memory map file
```

**Fix**: Check file permissions and size
```bash
# Verify file is readable
ls -l archive.zip

# Check available memory
free -h
```

## Resources

- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [Rustc Book](https://doc.rust-lang.org/rustc/)
- [Rust Compiler Error Index](https://doc.rust-lang.org/error-index.html)

Use this skill whenever you need to build the project or troubleshoot compilation errors.
