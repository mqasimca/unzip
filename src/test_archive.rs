//! Archive integrity testing
//!
//! This module provides ZIP archive integrity verification through CRC32 checksum
//! validation. It reads each file in the archive and verifies its checksum matches
//! the value stored in the ZIP central directory.
//!
//! # Features
//!
//! - CRC32 verification for all files
//! - Pattern-based file filtering
//! - Progress reporting during testing
//! - Detailed error reporting for corrupted files
//!
//! # Examples
//!
//! ```no_run
//! use std::fs::File;
//! use zip::ZipArchive;
//! use unzip::{Args, test_archive};
//! use clap::Parser;
//!
//! let file = File::open("archive.zip")?;
//! let mut archive = ZipArchive::new(file)?;
//! let args = Args::parse();
//! test_archive(&mut archive, &args)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use anyhow::{Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{Read, Seek};
use std::sync::atomic::{AtomicUsize, Ordering};
use zip::ZipArchive;

use crate::args::Args;
use crate::utils::PatternMatcher;

/// Test ZIP archive integrity by verifying CRC32 checksums for all files.
///
/// Reads each file in the archive and compares its calculated CRC32 checksum
/// against the value stored in the ZIP central directory. Files that fail
/// verification are reported with detailed error messages.
///
/// # Arguments
///
/// * `archive` - The ZIP archive to test
/// * `args` - Command-line arguments controlling:
///   - Pattern filters (test only matching files)
///   - Quiet mode (suppress progress output)
///
/// # Errors
///
/// Returns an error if:
/// - Any file's CRC32 checksum doesn't match (indicates corruption)
/// - A file cannot be read from the archive
/// - The number of errors exceeds zero (after testing all files)
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use zip::ZipArchive;
/// use unzip::{Args, test_archive};
/// use clap::Parser;
///
/// let file = File::open("archive.zip")?;
/// let mut archive = ZipArchive::new(file)?;
/// let args = Args::parse();
/// test_archive(&mut archive, &args)?;  // Returns Ok if all files valid
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn test_archive<R: Read + Seek>(archive: &mut ZipArchive<R>, args: &Args) -> Result<()> {
    let total_files = archive.len();
    let errors = AtomicUsize::new(0);
    let tested = AtomicUsize::new(0);
    let matcher = PatternMatcher::new(&args.patterns, &args.exclude, args.case_insensitive);
    let mut buffer = vec![0u8; 256 * 1024];

    let progress_bar = if args.quiet == 0 {
        let pb = ProgressBar::new(total_files as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} Testing [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    for i in 0..total_files {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if !matcher.should_extract(&name) {
            if let Some(ref pb) = progress_bar {
                pb.inc(1);
            }
            continue;
        }

        let mut hasher = crc32fast::Hasher::new();
        let mut read_error: Option<anyhow::Error> = None;
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => hasher.update(&buffer[..n]),
                Err(e) => {
                    read_error = Some(e.into());
                    break;
                },
            }
        }

        if let Some(e) = read_error {
            if args.quiet < 2 {
                eprintln!("error: {} - {}", name, e);
            }
            errors.fetch_add(1, Ordering::Relaxed);
        } else {
            let computed_crc = hasher.finalize();
            let stored_crc = file.crc32();

            if computed_crc != stored_crc {
                if args.quiet < 2 {
                    eprintln!(
                        "error: {} - CRC mismatch (stored: {:08x}, computed: {:08x})",
                        name, stored_crc, computed_crc
                    );
                }
                errors.fetch_add(1, Ordering::Relaxed);
            } else if args.quiet == 0
                && let Some(ref pb) = progress_bar
            {
                pb.println(format!("    testing: {}  OK", name));
            }
        }

        tested.fetch_add(1, Ordering::Relaxed);
        if let Some(ref pb) = progress_bar {
            pb.inc(1);
        }
    }

    if let Some(pb) = progress_bar {
        pb.finish_and_clear();
    }

    let error_count = errors.load(Ordering::Relaxed);
    let test_count = tested.load(Ordering::Relaxed);

    if args.quiet < 2 {
        if error_count == 0 {
            println!(
                "No errors detected in compressed data of {}.  {} files tested.",
                args.zipfile.display(),
                test_count
            );
        } else {
            println!(
                "{} error(s) detected in {}.  {} files tested.",
                error_count,
                args.zipfile.display(),
                test_count
            );
        }
    }

    if error_count > 0 {
        bail!("Archive test failed with {} errors", error_count);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use std::path::PathBuf;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    fn create_test_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = ZipWriter::new(Cursor::new(&mut buf));
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

            for (name, content) in files {
                if name.ends_with('/') {
                    zip.add_directory(*name, options).unwrap();
                } else {
                    zip.start_file(*name, options).unwrap();
                    zip.write_all(content).unwrap();
                }
            }
            zip.finish().unwrap();
        }
        buf
    }

    fn default_args() -> Args {
        Args {
            zipfile: PathBuf::from("test.zip"),
            output_dir: None,
            list_only: false,
            verbose: false,
            test: true,
            pipe: false,
            comment_only: false,
            zipinfo: None,
            overwrite: false,
            never_overwrite: false,
            freshen: false,
            update: false,
            junk_paths: false,
            case_insensitive: false,
            lowercase: false,
            no_timestamps: false,
            quiet: 2, // Suppress output in tests
            threads: None,
            password: None,
            patterns: vec![],
            exclude: vec![],
        }
    }

    #[test]
    fn test_archive_valid_files() {
        let zip_data = create_test_zip(&[
            ("test.txt", b"Test content"),
            ("file.rs", b"Rust file content"),
            ("data.bin", &[0u8, 1, 2, 3, 4, 5]),
        ]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();
        let args = default_args();

        // Should pass with no errors for valid archive
        let result = test_archive(&mut archive, &args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_archive_empty_archive() {
        let zip_data = create_test_zip(&[]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();
        let args = default_args();

        // Should handle empty archives gracefully
        let result = test_archive(&mut archive, &args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_archive_large_file() {
        let large_content = vec![b'X'; 5 * 1024 * 1024]; // 5MB

        let zip_data = create_test_zip(&[("small.txt", b"small"), ("large.bin", &large_content)]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();
        let args = default_args();

        // Should handle large files correctly
        let result = test_archive(&mut archive, &args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_archive_with_patterns() {
        let zip_data = create_test_zip(&[
            ("test.txt", b"Text file"),
            ("code.rs", b"Rust file"),
            ("data.json", b"JSON file"),
        ]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        let mut args = default_args();
        args.patterns = vec!["*.txt".to_string()];

        // Should test only matching files
        let result = test_archive(&mut archive, &args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_archive_with_exclude() {
        let zip_data = create_test_zip(&[
            ("test.txt", b"Text file"),
            ("debug.log", b"Log file"),
            ("data.bin", b"Binary file"),
        ]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        let mut args = default_args();
        args.exclude = vec!["*.log".to_string()];

        // Should test files except excluded ones
        let result = test_archive(&mut archive, &args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_archive_with_directories() {
        let zip_data = create_test_zip(&[
            ("dir1/", &[]),
            ("dir1/file.txt", b"Content"),
            ("dir2/", &[]),
            ("dir2/nested/", &[]),
            ("dir2/nested/deep.txt", b"Deep content"),
        ]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();
        let args = default_args();

        // Should handle directories correctly
        let result = test_archive(&mut archive, &args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_archive_zero_byte_files() {
        let zip_data = create_test_zip(&[
            ("empty1.txt", &[]),
            ("empty2.bin", &[]),
            ("nonempty.txt", b"Content"),
        ]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();
        let args = default_args();

        // Should handle zero-byte files correctly
        let result = test_archive(&mut archive, &args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_archive_binary_content() {
        let binary_data: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let zip_data = create_test_zip(&[("binary.bin", &binary_data)]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();
        let args = default_args();

        // Should handle binary content correctly
        let result = test_archive(&mut archive, &args);
        assert!(result.is_ok());
    }
}
