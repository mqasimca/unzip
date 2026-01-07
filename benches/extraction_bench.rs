//! Performance benchmarks for ZIP extraction
//!
//! Run with: cargo bench

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::io::{Cursor, Write};
use std::path::PathBuf;
use tempfile::TempDir;
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

use unzip::{Args, extract_archive};

/// Create a test ZIP archive with the specified number of files and size per file
fn create_test_archive(num_files: usize, bytes_per_file: usize) -> Vec<u8> {
    let mut buffer = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buffer));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    // Create dummy data (zeros for simplicity and compressibility)
    let file_data = vec![0u8; bytes_per_file];

    for i in 0..num_files {
        let filename = format!("file_{:04}.dat", i);
        zip.start_file(filename, options).unwrap();
        zip.write_all(&file_data).unwrap();
    }

    zip.finish().unwrap();
    buffer
}

/// Benchmark extraction of various archive sizes
fn bench_extract_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_small");

    // 10MB archive: 100 files @ 100KB each
    let num_files = 100;
    let bytes_per_file = 100 * 1024;
    let total_size = num_files * bytes_per_file;

    group.throughput(Throughput::Bytes(total_size as u64));

    let zip_data = create_test_archive(num_files, bytes_per_file);

    group.bench_function("10MB_100_files", |b| {
        b.iter(|| {
            let temp_dir = TempDir::new().unwrap();
            let cursor = Cursor::new(&zip_data);
            let mut archive = zip::ZipArchive::new(cursor).unwrap();

            let args = Args {
                zipfile: PathBuf::from("test.zip"),
                output_dir: Some(temp_dir.path().to_path_buf()),
                quiet: 2, // Suppress all output
                ..Default::default()
            };

            extract_archive(&mut archive, black_box(&args)).unwrap();
        });
    });

    group.finish();
}

/// Benchmark extraction of larger archives
fn bench_extract_medium(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_medium");
    group.sample_size(20); // Fewer samples for larger benchmarks

    // 100MB archive: 100 files @ 1MB each
    let num_files = 100;
    let bytes_per_file = 1024 * 1024;
    let total_size = num_files * bytes_per_file;

    group.throughput(Throughput::Bytes(total_size as u64));

    let zip_data = create_test_archive(num_files, bytes_per_file);

    group.bench_function("100MB_100_files", |b| {
        b.iter(|| {
            let temp_dir = TempDir::new().unwrap();
            let cursor = Cursor::new(&zip_data);
            let mut archive = zip::ZipArchive::new(cursor).unwrap();

            let args = Args {
                zipfile: PathBuf::from("test.zip"),
                output_dir: Some(temp_dir.path().to_path_buf()),
                quiet: 2,
                ..Default::default()
            };

            extract_archive(&mut archive, black_box(&args)).unwrap();
        });
    });

    group.finish();
}

/// Benchmark extraction of many small files
fn bench_extract_many_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_many_small");

    // 10MB archive: 1000 files @ 10KB each
    let num_files = 1000;
    let bytes_per_file = 10 * 1024;
    let total_size = num_files * bytes_per_file;

    group.throughput(Throughput::Bytes(total_size as u64));

    let zip_data = create_test_archive(num_files, bytes_per_file);

    group.bench_function("10MB_1000_files", |b| {
        b.iter(|| {
            let temp_dir = TempDir::new().unwrap();
            let cursor = Cursor::new(&zip_data);
            let mut archive = zip::ZipArchive::new(cursor).unwrap();

            let args = Args {
                zipfile: PathBuf::from("test.zip"),
                output_dir: Some(temp_dir.path().to_path_buf()),
                quiet: 2,
                ..Default::default()
            };

            extract_archive(&mut archive, black_box(&args)).unwrap();
        });
    });

    group.finish();
}

/// Benchmark glob pattern matching performance
fn bench_glob_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("glob_filtering");

    // Create archive with mixed file types
    let zip_data = {
        let mut buffer = Vec::new();
        let mut zip = ZipWriter::new(Cursor::new(&mut buffer));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let file_data = vec![0u8; 1024];

        for i in 0..100 {
            for ext in &["txt", "rs", "md", "json", "log"] {
                let filename = format!("file_{:04}.{}", i, ext);
                zip.start_file(filename, options).unwrap();
                zip.write_all(&file_data).unwrap();
            }
        }

        zip.finish().unwrap();
        buffer
    };

    group.bench_function("filter_single_pattern", |b| {
        b.iter(|| {
            let temp_dir = TempDir::new().unwrap();
            let cursor = Cursor::new(&zip_data);
            let mut archive = zip::ZipArchive::new(cursor).unwrap();

            let args = Args {
                zipfile: PathBuf::from("test.zip"),
                output_dir: Some(temp_dir.path().to_path_buf()),
                patterns: vec!["*.rs".to_string()],
                quiet: 2,
                ..Default::default()
            };

            extract_archive(&mut archive, black_box(&args)).unwrap();
        });
    });

    group.bench_function("filter_multiple_patterns", |b| {
        b.iter(|| {
            let temp_dir = TempDir::new().unwrap();
            let cursor = Cursor::new(&zip_data);
            let mut archive = zip::ZipArchive::new(cursor).unwrap();

            let args = Args {
                zipfile: PathBuf::from("test.zip"),
                output_dir: Some(temp_dir.path().to_path_buf()),
                patterns: vec!["*.rs".to_string(), "*.md".to_string()],
                quiet: 2,
                ..Default::default()
            };

            extract_archive(&mut archive, black_box(&args)).unwrap();
        });
    });

    group.finish();
}

/// Benchmark glob pattern matching only (no extraction)
fn bench_glob_match(c: &mut Criterion) {
    use unzip::glob_match;

    let mut group = c.benchmark_group("glob_match");

    let patterns = vec![
        ("simple_wildcard", "*.txt", "file.txt"),
        ("recursive_wildcard", "**/*.rs", "src/main.rs"),
        ("question_mark", "file?.dat", "file1.dat"),
        ("no_wildcard", "exact.txt", "exact.txt"),
    ];

    for (name, pattern, text) in patterns {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &(pattern, text),
            |b, &(p, t)| {
                b.iter(|| {
                    black_box(glob_match(p, t));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_extract_small,
    bench_extract_medium,
    bench_extract_many_small,
    bench_glob_filtering,
    bench_glob_match
);
criterion_main!(benches);
