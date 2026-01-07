---
name: rust-benchmarking
description: Run and create Rust benchmarks for performance measurement. Use when measuring performance or comparing implementations.
allowed-tools: Bash(cargo:*), Bash(make:*), Bash(time:*), Bash(hyperfine:*), Read, Edit, Grep, Glob
---

# Rust Benchmarking Skill

Performance measurement and benchmarking for the unzip project.

## Quick Benchmarks

### Using `time` (Simple)

```bash
# Time a single extraction
time ./target/release/unzip archive.zip

# Time with specific output format
/usr/bin/time -v ./target/release/unzip archive.zip

# Compare implementations
time ./old_unzip archive.zip
time ./target/release/unzip archive.zip
```

### Using `hyperfine` (Recommended)

```bash
# Install hyperfine
cargo install hyperfine

# Simple benchmark
hyperfine './target/release/unzip archive.zip'

# Compare implementations
hyperfine --warmup 1 \
  'info-zip unzip archive.zip' \
  './target/release/unzip archive.zip'

# Multiple runs for statistical significance
hyperfine --warmup 2 --runs 10 \
  './target/release/unzip large.zip'

# Export results
hyperfine --export-json results.json \
  './target/release/unzip archive.zip'
```

## Creating Benchmark Tests

### Using Criterion (Recommended)

Add to `Cargo.toml`:
```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "extraction_bench"
harness = false
```

Create `benches/extraction_bench.rs`:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::fs::File;
use zip::ZipArchive;
use unzip::extract_archive;

fn bench_extract_small(c: &mut Criterion) {
    c.bench_function("extract_small_10mb", |b| {
        b.iter(|| {
            let file = File::open("test_data/small.zip").unwrap();
            let mut archive = ZipArchive::new(file).unwrap();
            extract_archive(black_box(&mut archive), &args())
        });
    });
}

fn bench_extract_large(c: &mut Criterion) {
    c.bench_function("extract_large_1gb", |b| {
        b.iter(|| {
            let file = File::open("test_data/large.zip").unwrap();
            let mut archive = ZipArchive::new(file).unwrap();
            extract_archive(black_box(&mut archive), &args())
        });
    });
}

fn bench_extract_many_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_many");

    for num_files in [100, 1000, 10000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_files),
            &num_files,
            |b, &n| {
                b.iter(|| {
                    let file = File::open(format!("test_data/{}_files.zip", n)).unwrap();
                    let mut archive = ZipArchive::new(file).unwrap();
                    extract_archive(black_box(&mut archive), &args())
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_extract_small, bench_extract_large, bench_extract_many_files);
criterion_main!(benches);
```

### Running Criterion Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench extract_small

# Run with custom baseline
cargo bench --save-baseline my-baseline

# Compare against baseline
cargo bench --baseline my-baseline

# Generate HTML reports
cargo bench
open target/criterion/report/index.html
```

## Benchmark Scenarios for Unzip

### 1. File Size Variations

```bash
# Create test archives
dd if=/dev/urandom of=test_10mb.dat bs=1M count=10
zip test_10mb.zip test_10mb.dat

dd if=/dev/urandom of=test_100mb.dat bs=1M count=100
zip test_100mb.zip test_100mb.dat

dd if=/dev/urandom of=test_1gb.dat bs=1M count=1024
zip test_1gb.zip test_1gb.dat

# Benchmark each
hyperfine --warmup 1 \
  './target/release/unzip test_10mb.zip' \
  './target/release/unzip test_100mb.zip' \
  './target/release/unzip test_1gb.zip'
```

### 2. File Count Variations

```bash
# Many small files
mkdir -p test_files
for i in {1..1000}; do
    dd if=/dev/urandom of=test_files/file_$i.dat bs=1K count=10
done
zip -r test_1000_files.zip test_files/

# Few large files
mkdir -p test_large
for i in {1..10}; do
    dd if=/dev/urandom of=test_large/file_$i.dat bs=1M count=100
done
zip -r test_10_large.zip test_large/

# Benchmark
hyperfine --warmup 1 \
  './target/release/unzip test_1000_files.zip' \
  './target/release/unzip test_10_large.zip'
```

### 3. Compression Ratio Variations

```bash
# Uncompressed (stored)
zip -0 uncompressed.zip large.dat

# Maximum compression
zip -9 compressed.zip large.dat

# Benchmark
hyperfine --warmup 1 \
  './target/release/unzip uncompressed.zip' \
  './target/release/unzip compressed.zip'
```

### 4. Memory Mapping Threshold

The unzip project uses mmap for files >1MB (main.rs:30):

```bash
# Below threshold (no mmap)
dd if=/dev/urandom of=small.dat bs=1K count=512
zip small.zip small.dat

# Above threshold (uses mmap)
dd if=/dev/urandom of=large.dat bs=1M count=10
zip large.zip large.dat

# Benchmark
hyperfine --warmup 1 \
  './target/release/unzip small.zip' \
  './target/release/unzip large.zip'
```

## Performance Profiling

### CPU Profiling with `perf` (Linux)

```bash
# Record profile
perf record -g ./target/release/unzip large.zip

# View report
perf report

# Generate flamegraph
perf script | stackcollapse-perf.pl | flamegraph.pl > flame.svg
open flame.svg
```

### Flamegraph (Easier)

```bash
# Install
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --release -- large.zip

# Opens flamegraph.svg in browser
```

### Memory Profiling with Valgrind

```bash
# Install valgrind
sudo apt-get install valgrind

# Profile memory usage
valgrind --tool=massif ./target/release/unzip large.zip

# Analyze results
ms_print massif.out.* | less
```

### Memory Profiling with Heaptrack (Linux)

```bash
# Install
sudo apt-get install heaptrack

# Profile
heaptrack ./target/release/unzip large.zip

# Analyze (opens GUI)
heaptrack_gui heaptrack.unzip.*.gz
```

## Key Performance Metrics

### Throughput

```bash
# Calculate MB/s
FILE_SIZE=$(stat -f%z archive.zip)  # macOS
# Or
FILE_SIZE=$(stat -c%s archive.zip)  # Linux

time ./target/release/unzip archive.zip
# Manually calculate: FILE_SIZE / elapsed_time_seconds / 1024 / 1024
```

### Memory Usage

```bash
# Peak memory (Linux)
/usr/bin/time -v ./target/release/unzip large.zip | grep "Maximum resident set size"

# Monitor in real-time
while true; do
    ps aux | grep unzip | grep -v grep
    sleep 0.1
done
```

### CPU Utilization

```bash
# Monitor CPU usage
top -p $(pgrep unzip)

# Or use htop
htop -p $(pgrep unzip)
```

## Benchmark Results Interpretation

### Criterion Output

```
extract_small_10mb      time:   [95.234 ms 95.891 ms 96.612 ms]
                        change: [-2.1234% -1.2456% -0.3421%] (p = 0.01 < 0.05)
                        Performance has improved.
```

- **Time**: Median execution time with confidence interval
- **Change**: Comparison to previous run (if available)
- **p-value**: Statistical significance (< 0.05 is significant)

### What to Look For

**Good Performance:**
- Throughput > 500 MB/s for uncompressed
- Throughput > 100 MB/s for compressed
- Memory usage < 500 MB for 1GB archive
- CPU usage 20-40% (single-threaded) or 80-100% (multi-threaded)

**Red Flags:**
- Throughput decreases with larger files (should scale linearly)
- Memory usage grows unbounded
- CPU usage < 10% (I/O bottleneck)
- High variance between runs (>10%)

## Comparing Implementations

### Before/After Optimization

```bash
# Save baseline
git stash  # Save current changes
cargo bench --save-baseline before

# Apply optimization
git stash pop

# Compare
cargo bench --baseline before
```

### Against Info-ZIP

```bash
# Create test archive
dd if=/dev/urandom of=large.dat bs=1M count=1024
zip test.zip large.dat

# Compare
hyperfine --warmup 1 --runs 5 \
  --command-name "info-zip" "unzip -q -o test.zip" \
  --command-name "rust-unzip" "./target/release/unzip -q -o test.zip"
```

Expected output (from README.md):
```
Benchmark 1: info-zip
  Time (mean ± σ):      4.92 s ±  0.08 s    [User: 1.2 s, System: 3.7 s]
  Range (min … max):    4.84 s …  5.03 s    5 runs

Benchmark 2: rust-unzip
  Time (mean ± σ):     952.3 ms ± 12.4 ms    [User: 234.1 ms, System: 687.2 ms]
  Range (min … max):   940.1 ms … 968.7 ms    5 runs

Summary
  'rust-unzip' ran
    5.17 ± 0.09 times faster than 'info-zip'
```

## Optimization Workflow

### 1. Establish Baseline

```bash
cargo build --release
cargo bench --save-baseline baseline
```

### 2. Profile to Find Bottleneck

```bash
cargo flamegraph --release -- large.zip
# Identify hot functions
```

### 3. Implement Optimization

Edit code based on profiling results.

### 4. Benchmark Improvement

```bash
cargo bench --baseline baseline
```

### 5. Verify Correctness

```bash
cargo test
```

### 6. Document Results

Add to commit message or CHANGELOG:
```
perf: Optimize extraction loop

Reduces extraction time by 15% for large archives.

Benchmark (1GB archive):
- Before: 952ms
- After: 809ms
- Improvement: 15%
```

## Benchmarking Best Practices

### Do's ✓

- Always benchmark in release mode (`--release`)
- Run multiple iterations (>5) for statistical significance
- Warm up before measuring (1-2 runs)
- Use realistic test data
- Control system state (close other programs)
- Document system specs with results
- Use `black_box()` to prevent compiler optimizations
- Measure multiple metrics (time, memory, CPU)

### Don'ts ✗

- Don't benchmark debug builds
- Don't run single iteration
- Don't benchmark on busy system
- Don't use trivial test data
- Don't trust first run (cold cache)
- Don't forget to clean up test artifacts
- Don't optimize without measuring first
- Don't micro-optimize without proof

## Quick Reference

| Tool | Purpose | Install |
|------|---------|---------|
| `time` | Simple timing | Built-in |
| `hyperfine` | Statistical benchmarking | `cargo install hyperfine` |
| `criterion` | Rust micro-benchmarks | Add to Cargo.toml |
| `flamegraph` | CPU profiling | `cargo install flamegraph` |
| `perf` | Linux profiling | `apt-get install linux-tools` |
| `valgrind` | Memory profiling | `apt-get install valgrind` |

## Makefile Integration

Add to Makefile:
```makefile
bench:
	cargo bench --release

bench-baseline:
	cargo bench --save-baseline main

bench-compare:
	cargo bench --baseline main
```

## Performance Targets for Unzip

Based on README.md benchmarks:

| Archive Size | Target Time | Target Throughput |
|-------------|-------------|-------------------|
| 10 MB | < 10 ms | > 1 GB/s |
| 100 MB | < 100 ms | > 1 GB/s |
| 1 GB | < 1000 ms | > 1 GB/s |
| 10 GB | < 10 s | > 1 GB/s |

**Current Performance:** ~5x faster than Info-ZIP (~1.05 GB/s)

Use this skill to measure performance, create benchmarks, and validate optimizations.
