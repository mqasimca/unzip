//! Archive extraction functionality
//!
//! This module provides the core ZIP archive extraction logic with support for:
//! - Multiple overwrite modes (always, never, freshen, update)
//! - Pattern-based file inclusion/exclusion
//! - Progress reporting with file counts and sizes
//! - Preservation of file timestamps and Unix permissions
//! - Extraction to stdout for piping
//!
//! # Performance
//!
//! Optimized for throughput using:
//! - 256KB I/O buffers matching typical filesystem block sizes
//! - Linux kernel hints (fallocate, fadvise) when available
//! - Minimal memory allocations through buffer reuse
//!
//! # Examples
//!
//! ```no_run
//! use std::fs::File;
//! use zip::ZipArchive;
//! use unzip::{Args, extract_archive};
//! use clap::Parser;
//!
//! let file = File::open("archive.zip")?;
//! let mut archive = ZipArchive::new(file)?;
//! let args = Args::parse();
//! extract_archive(&mut archive, &args)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use zip::ZipArchive;

use crate::args::Args;
use crate::linux::{fadvise_dontneed, preallocate_file};
use crate::password::{get_password, is_password_error, prompt_for_password};
use crate::utils::{datetime_to_filetime, datetime_to_system_time, format_size, should_extract};

/// Buffer size for file I/O (256KB for better throughput)
const BUFFER_SIZE: usize = 256 * 1024;

/// Decision on whether to overwrite an existing file
#[derive(Debug, PartialEq, Eq)]
enum OverwriteDecision {
    /// Overwrite the existing file
    Overwrite,
    /// Skip extraction and show a message
    Skip,
    /// Skip extraction quietly (no message)
    SkipQuietly,
}

/// Finalize an extracted file by setting modification time and permissions
///
/// # Arguments
///
/// * `outpath` - Path to the extracted file
/// * `modified_time` - Optional modification time from archive
/// * `unix_mode` - Optional Unix permissions mode
/// * `no_timestamps` - Skip timestamp restoration if true
///
/// # Errors
///
/// This function logs errors but does not fail the extraction process
fn finalize_extracted_file(
    outpath: &std::path::Path,
    modified_time: Option<zip::DateTime>,
    unix_mode: Option<u32>,
    no_timestamps: bool,
) {
    if !no_timestamps && let Some(dt) = modified_time {
        let mtime = datetime_to_filetime(dt);
        filetime::set_file_mtime(outpath, mtime).ok();
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Some(mode) = unix_mode {
            fs::set_permissions(outpath, fs::Permissions::from_mode(mode)).ok();
        }
    }

    // Suppress unused variable warning on non-Unix platforms
    #[cfg(not(unix))]
    {
        let _ = unix_mode;
    }
}

/// Extract a single file from the archive to the filesystem
///
/// # Arguments
///
/// * `file` - The zip file entry to extract
/// * `outpath` - Destination path for the extracted file
/// * `buffer` - Reusable buffer for I/O operations
///
/// # Returns
///
/// Returns the number of bytes written
///
/// # Errors
///
/// Returns an error if file creation, writing, or finalization fails
fn extract_single_file(
    file: &mut zip::read::ZipFile,
    outpath: &std::path::Path,
    buffer: &mut [u8],
) -> Result<u64> {
    let size = file.size();

    let outfile = File::create(outpath)
        .with_context(|| format!("Failed to create file: {}", outpath.display()))?;

    // Linux optimization: pre-allocate disk space to avoid fragmentation
    if size > 0 {
        preallocate_file(&outfile, size).ok();
    }

    // Use larger buffer for better throughput
    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, outfile);

    // Manual copy with reused buffer for less allocation
    let mut bytes_written = 0u64;
    loop {
        let bytes_read = file.read(buffer)?;
        if bytes_read == 0 {
            break;
        }
        writer.write_all(&buffer[..bytes_read])?;
        bytes_written += bytes_read as u64;
    }

    let inner_file = writer.into_inner()?;

    // Linux optimization: tell kernel we're done with this file's cache
    fadvise_dontneed(&inner_file, 0, size);

    Ok(bytes_written)
}

/// Determine whether to overwrite an existing file based on extraction args
///
/// # Arguments
///
/// * `outpath` - Path to the file that may exist
/// * `args` - Command-line arguments with overwrite flags
/// * `archive_modified` - Modification time from the archive file
///
/// # Returns
///
/// Returns `OverwriteDecision` indicating whether to overwrite, skip with message, or skip quietly
fn should_overwrite_file(
    outpath: &std::path::Path,
    args: &Args,
    archive_modified: Option<zip::DateTime>,
) -> OverwriteDecision {
    if !outpath.exists() {
        if args.freshen {
            return OverwriteDecision::SkipQuietly;
        }
        return OverwriteDecision::Overwrite;
    }

    if args.freshen || args.update {
        if let Ok(meta) = outpath.metadata()
            && let Ok(disk_mtime) = meta.modified()
            && let Some(archive_mtime) = archive_modified
        {
            let archive_time = datetime_to_system_time(archive_mtime);
            if archive_time <= disk_mtime {
                return OverwriteDecision::SkipQuietly;
            }
        }
        return OverwriteDecision::Overwrite;
    }

    if args.never_overwrite {
        return OverwriteDecision::Skip;
    } else if args.overwrite {
        return OverwriteDecision::Overwrite;
    }

    OverwriteDecision::Skip
}

/// Extract files to stdout for piping to other commands.
///
///Writes file contents directly to stdout without creating files on disk.
/// Directories are skipped. Multiple files are concatenated sequentially.
///
/// # Arguments
///
/// * `archive` - The ZIP archive to extract from
/// * `args` - Command-line arguments controlling which files to extract
///
/// # Errors
///
/// Returns an error if:
/// - A file cannot be read from the archive
/// - Writing to stdout fails
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use zip::ZipArchive;
/// use unzip::Args;
/// use unzip::extract::extract_to_pipe;
/// use clap::Parser;
///
/// let file = File::open("archive.zip")?;
/// let mut archive = ZipArchive::new(file)?;
/// let args = Args::parse();
/// extract_to_pipe(&mut archive, &args)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn extract_to_pipe<R: Read + Seek>(archive: &mut ZipArchive<R>, args: &Args) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    let password = Mutex::new(get_password(args.password.as_deref(), args.quiet)?);

    for i in 0..archive.len() {
        // First, get file info without extracting
        let is_dir = {
            let f = archive.by_index(i)?;
            f.is_dir()
        };

        if is_dir {
            continue;
        }

        let name = {
            let f = archive.by_index(i)?;
            f.name().to_string()
        };

        if !should_extract(&name, &args.patterns, &args.exclude, args.case_insensitive) {
            continue;
        }

        // Try to extract file - handle encryption
        let mut file;
        let result = archive.by_index(i);
        if let Ok(f) = result {
            file = f;
        } else {
            // Extract error information before dropping result
            let err_str = match &result {
                Err(e) => e.to_string(),
                Ok(_) => unreachable!(),
            };
            let is_pwd_error = is_password_error(&err_str);
            drop(result); // Drop result to release the borrow

            if is_pwd_error {
                // Error is password-related, need to decrypt
                let mut pwd = password.lock().unwrap();
                if pwd.is_none() {
                    if args.quiet == 0 {
                        eprintln!("Encrypted file detected: {}", name);
                    }
                    *pwd = Some(prompt_for_password()?);
                }
                let pwd_bytes = pwd.clone();
                drop(pwd);

                if let Some(ref pwd) = pwd_bytes {
                    file = archive
                        .by_index_decrypt(i, pwd)
                        .with_context(|| format!("Failed to decrypt {}", name))?;
                } else {
                    bail!("Password required but not available for file: {}", name);
                }
            } else {
                bail!("Failed to read file: {}", name);
            }
        }

        io::copy(&mut file, &mut stdout_lock)
            .with_context(|| format!("Failed to write {} to stdout", name))?;
    }

    Ok(())
}

/// Extract archive contents to the filesystem with Linux optimizations.
///
/// This is the main extraction function that handles all ZIP archive extraction with
/// support for multiple overwrite modes, pattern filtering, progress reporting, and
/// file metadata preservation.
///
/// # Arguments
///
/// * `archive` - The ZIP archive to extract from
/// * `args` - Command-line arguments controlling extraction behavior including:
///   - Output directory (`-d`)
///   - Overwrite mode (`-o`, `-n`, `-f`, `-u`)
///   - Pattern filters (include/exclude)
///   - Directory flattening (`-j`)
///   - Quiet mode (`-q`)
///
/// # Errors
///
/// Returns an error if:
/// - The output directory cannot be created
/// - A file cannot be extracted due to permissions or disk space
/// - Directory traversal is detected in a file path
/// - File timestamps cannot be set
///
/// # Performance
///
/// Uses several optimizations for throughput:
/// - 256KB I/O buffers for efficient disk writes
/// - Linux fallocate() to pre-allocate space and prevent fragmentation
/// - Linux fadvise() to hint sequential access patterns
/// - Buffer reuse to minimize allocations
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use zip::ZipArchive;
/// use unzip::{Args, extract_archive};
/// use clap::Parser;
///
/// let file = File::open("archive.zip")?;
/// let mut archive = ZipArchive::new(file)?;
/// let args = Args::parse();
/// extract_archive(&mut archive, &args)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn extract_archive<R: Read + Seek>(archive: &mut ZipArchive<R>, args: &Args) -> Result<()> {
    let output_dir = args.output_dir.clone().unwrap_or_else(|| PathBuf::from("."));

    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).with_context(|| {
            format!("Failed to create output directory: {}", output_dir.display())
        })?;
    }

    let total_files = archive.len();
    let extracted = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let total_bytes = AtomicU64::new(0);

    let password = Mutex::new(get_password(args.password.as_deref(), args.quiet)?);

    let progress_bar = if args.quiet == 0 {
        let pb = ProgressBar::new(total_files as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )?
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    let mut file_infos: Vec<(usize, String, bool, u64, Option<zip::DateTime>)> = Vec::new();

    for i in 0..total_files {
        let file = archive.by_index(i)?;
        let name = file.name().to_string();
        let is_dir = file.is_dir();
        let size = file.size();
        let mtime = file.last_modified();
        file_infos.push((i, name, is_dir, size, mtime));
    }

    // Track directories for timestamp restoration after extraction
    let mut directories: Vec<(PathBuf, Option<zip::DateTime>)> = Vec::new();

    // Must be sequential to avoid race conditions when creating nested directories
    for (_, name, is_dir, _, mtime) in &file_infos {
        if !*is_dir {
            continue;
        }

        if args.junk_paths {
            continue; // Skip directories in junk mode
        }

        let name = if args.lowercase {
            name.to_lowercase()
        } else {
            name.clone()
        };
        let outpath = output_dir.join(&name);

        fs::create_dir_all(&outpath)
            .with_context(|| format!("Failed to create directory: {}", outpath.display()))?;

        // Track directory for later timestamp restoration
        directories.push((outpath, *mtime));
    }

    let mut buffer = vec![0u8; BUFFER_SIZE];

    'main_loop: for (i, name, is_dir, size, mtime) in file_infos {
        if is_dir {
            if let Some(ref pb) = progress_bar {
                pb.inc(1);
            }
            continue;
        }

        if !should_extract(&name, &args.patterns, &args.exclude, args.case_insensitive) {
            if let Some(ref pb) = progress_bar {
                pb.inc(1);
            }
            skipped.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        // Try to extract file - handle encryption
        let mut file;
        let result = archive.by_index(i);
        if let Ok(f) = result {
            file = f;
        } else {
            // Extract error information before dropping result
            let err_str = match &result {
                Err(e) => e.to_string(),
                Ok(_) => unreachable!(),
            };
            let is_pwd_error = is_password_error(&err_str);
            drop(result); // Drop result to release the borrow

            if is_pwd_error {
                // Ensure we have a password
                let mut pwd = password.lock().unwrap();
                if pwd.is_none() {
                    if args.quiet == 0 {
                        if let Some(ref pb) = progress_bar {
                            pb.println(format!("Encrypted file detected: {}", name));
                        } else {
                            eprintln!("Encrypted file detected: {}", name);
                        }
                    }
                    *pwd = Some(prompt_for_password()?);
                }
                let pwd_bytes = pwd.clone();
                drop(pwd);

                // Try to decrypt with password
                if let Some(ref pwd) = pwd_bytes {
                    match archive.by_index_decrypt(i, pwd) {
                        Ok(f) => file = f,
                        Err(_e) => {
                            if args.quiet < 2 {
                                if let Some(ref pb) = progress_bar {
                                    pb.println(format!("    error: {} - Invalid password", name));
                                } else {
                                    eprintln!("error: {} - Invalid password", name);
                                }
                            }
                            if let Some(ref pb) = progress_bar {
                                pb.inc(1);
                            }
                            skipped.fetch_add(1, Ordering::Relaxed);
                            continue 'main_loop;
                        },
                    }
                } else {
                    if args.quiet < 2 {
                        if let Some(ref pb) = progress_bar {
                            pb.println(format!("    error: {} - Password required", name));
                        } else {
                            eprintln!("error: {} - Password required", name);
                        }
                    }
                    if let Some(ref pb) = progress_bar {
                        pb.inc(1);
                    }
                    skipped.fetch_add(1, Ordering::Relaxed);
                    continue 'main_loop;
                }
            } else {
                bail!("Failed to read file: {}", name);
            }
        }

        let outpath = if args.junk_paths {
            let filename = std::path::Path::new(&name)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or(name.clone());
            let filename = if args.lowercase {
                filename.to_lowercase()
            } else {
                filename
            };
            output_dir.join(filename)
        } else {
            let name = if args.lowercase {
                name.to_lowercase()
            } else {
                name.clone()
            };
            match file.enclosed_name() {
                Some(_) => output_dir.join(&name),
                None => {
                    if let Some(ref pb) = progress_bar {
                        pb.inc(1);
                    }
                    continue;
                },
            }
        };

        if let Some(parent) = outpath.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let decision = should_overwrite_file(&outpath, args, mtime);

        match decision {
            OverwriteDecision::Skip => {
                if args.quiet == 0
                    && let Some(ref pb) = progress_bar
                {
                    let msg = if args.never_overwrite {
                        format!("    skipping: {} (already exists)", name)
                    } else {
                        format!("    skipping: {} (use -o to overwrite)", name)
                    };
                    pb.println(msg);
                }
                if let Some(ref pb) = progress_bar {
                    pb.inc(1);
                }
                skipped.fetch_add(1, Ordering::Relaxed);
                continue;
            },
            OverwriteDecision::SkipQuietly => {
                if let Some(ref pb) = progress_bar {
                    pb.inc(1);
                }
                skipped.fetch_add(1, Ordering::Relaxed);
                continue;
            },
            OverwriteDecision::Overwrite => {},
        }

        let unix_mode = {
            #[cfg(unix)]
            {
                file.unix_mode()
            }
            #[cfg(not(unix))]
            {
                None
            }
        };

        extract_single_file(&mut file, &outpath, &mut buffer)?;

        finalize_extracted_file(&outpath, mtime, unix_mode, args.no_timestamps);

        if args.quiet == 0
            && let Some(ref pb) = progress_bar
        {
            pb.println(format!("  extracting: {}", name));
        }

        extracted.fetch_add(1, Ordering::Relaxed);
        total_bytes.fetch_add(size, Ordering::Relaxed);

        if let Some(ref pb) = progress_bar {
            pb.inc(1);
        }
    }

    // Restore directory timestamps after all files extracted
    // This must be done last because extracting files updates directory mtimes
    if !args.no_timestamps {
        for (dir_path, mtime) in directories.iter().rev() {
            if let Some(dt) = mtime {
                let filetime_mtime = datetime_to_filetime(*dt);
                filetime::set_file_mtime(dir_path, filetime_mtime).ok();
            }
        }
    }

    if let Some(pb) = progress_bar {
        pb.finish_and_clear();
    }

    let extract_count = extracted.load(Ordering::Relaxed);
    let skip_count = skipped.load(Ordering::Relaxed);
    let bytes = total_bytes.load(Ordering::Relaxed);

    if args.quiet == 0 {
        println!(
            "Extracted {} files ({}) to {}",
            extract_count,
            format_size(bytes),
            output_dir.display()
        );
        if skip_count > 0 {
            println!("Skipped {} files", skip_count);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
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
            test: false,
            pipe: false,
            comment_only: false,
            zipinfo: None,
            overwrite: true,
            never_overwrite: false,
            freshen: false,
            update: false,
            junk_paths: false,
            case_insensitive: false,
            lowercase: false,
            no_timestamps: false,
            quiet: 2,
            threads: None,
            password: None,
            patterns: vec![],
            exclude: vec![],
        }
    }

    #[test]
    fn test_zip_extract_to_tempdir() {
        let zip_data = create_test_zip(&[
            ("test.txt", b"Test content"),
            ("subdir/nested.txt", b"Nested content"),
        ]);

        let temp_dir = tempfile::tempdir().unwrap();
        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        let mut args = default_args();
        args.output_dir = Some(temp_dir.path().to_path_buf());

        extract_archive(&mut archive, &args).unwrap();

        let test_file = temp_dir.path().join("test.txt");
        assert!(test_file.exists());
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "Test content");

        let nested_file = temp_dir.path().join("subdir/nested.txt");
        assert!(nested_file.exists());
        assert_eq!(fs::read_to_string(&nested_file).unwrap(), "Nested content");
    }

    #[test]
    fn test_zip_extract_with_pattern() {
        let zip_data = create_test_zip(&[
            ("file.txt", b"Text file"),
            ("file.rs", b"Rust file"),
            ("src/main.rs", b"Main rust"),
            ("doc/readme.txt", b"Readme"),
        ]);

        let temp_dir = tempfile::tempdir().unwrap();
        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        let mut args = default_args();
        args.output_dir = Some(temp_dir.path().to_path_buf());
        args.patterns = vec!["*.txt".to_string()];

        extract_archive(&mut archive, &args).unwrap();

        assert!(temp_dir.path().join("file.txt").exists());
        assert!(!temp_dir.path().join("file.rs").exists());
        assert!(!temp_dir.path().join("src/main.rs").exists());
        assert!(!temp_dir.path().join("doc/readme.txt").exists());
    }

    #[test]
    fn test_zip_extract_with_exclude() {
        let zip_data = create_test_zip(&[
            ("file.txt", b"Text file"),
            ("file.rs", b"Rust file"),
            ("debug.log", b"Log file"),
        ]);

        let temp_dir = tempfile::tempdir().unwrap();
        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        let mut args = default_args();
        args.output_dir = Some(temp_dir.path().to_path_buf());
        args.exclude = vec!["*.log".to_string()];

        extract_archive(&mut archive, &args).unwrap();

        assert!(temp_dir.path().join("file.txt").exists());
        assert!(temp_dir.path().join("file.rs").exists());
        assert!(!temp_dir.path().join("debug.log").exists());
    }

    #[test]
    fn test_zip_extract_junk_paths() {
        let zip_data = create_test_zip(&[("deep/nested/path/file.txt", b"Content")]);

        let temp_dir = tempfile::tempdir().unwrap();
        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        let mut args = default_args();
        args.output_dir = Some(temp_dir.path().to_path_buf());
        args.junk_paths = true;

        extract_archive(&mut archive, &args).unwrap();

        // File should be in root, not nested
        assert!(temp_dir.path().join("file.txt").exists());
        assert!(!temp_dir.path().join("deep").exists());
    }

    #[test]
    fn test_zip_extract_lowercase() {
        let zip_data = create_test_zip(&[("FILE.TXT", b"Content"), ("Dir/NESTED.RS", b"Rust")]);

        let temp_dir = tempfile::tempdir().unwrap();
        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        let mut args = default_args();
        args.output_dir = Some(temp_dir.path().to_path_buf());
        args.lowercase = true;

        extract_archive(&mut archive, &args).unwrap();

        assert!(temp_dir.path().join("file.txt").exists());
        assert!(temp_dir.path().join("dir/nested.rs").exists());
    }

    #[test]
    fn test_zip_no_overwrite() {
        let zip_data = create_test_zip(&[("test.txt", b"New content")]);

        let temp_dir = tempfile::tempdir().unwrap();

        let existing_file = temp_dir.path().join("test.txt");
        fs::write(&existing_file, "Original content").unwrap();

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        let mut args = default_args();
        args.output_dir = Some(temp_dir.path().to_path_buf());
        args.overwrite = false;
        args.never_overwrite = true;

        extract_archive(&mut archive, &args).unwrap();

        assert_eq!(fs::read_to_string(&existing_file).unwrap(), "Original content");
    }

    #[test]
    fn test_zip_overwrite() {
        let zip_data = create_test_zip(&[("test.txt", b"New content")]);

        let temp_dir = tempfile::tempdir().unwrap();

        let existing_file = temp_dir.path().join("test.txt");
        fs::write(&existing_file, "Original content").unwrap();

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        let mut args = default_args();
        args.output_dir = Some(temp_dir.path().to_path_buf());
        args.overwrite = true;

        extract_archive(&mut archive, &args).unwrap();

        assert_eq!(fs::read_to_string(&existing_file).unwrap(), "New content");
    }

    #[test]
    fn test_zip_empty_archive() {
        let zip_data = create_test_zip(&[]);

        let temp_dir = tempfile::tempdir().unwrap();
        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        assert_eq!(archive.len(), 0);

        let mut args = default_args();
        args.output_dir = Some(temp_dir.path().to_path_buf());

        extract_archive(&mut archive, &args).unwrap();
    }

    #[test]
    fn test_zip_binary_content() {
        let binary_data: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let zip_data = create_test_zip(&[("binary.bin", &binary_data)]);

        let temp_dir = tempfile::tempdir().unwrap();
        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        let mut args = default_args();
        args.output_dir = Some(temp_dir.path().to_path_buf());

        extract_archive(&mut archive, &args).unwrap();

        let extracted = fs::read(temp_dir.path().join("binary.bin")).unwrap();
        assert_eq!(extracted, binary_data);
    }

    #[test]
    fn test_should_overwrite_file_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("nonexistent.txt");
        let args = default_args();

        let decision = should_overwrite_file(&path, &args, None);
        assert_eq!(decision, OverwriteDecision::Overwrite);
    }

    #[test]
    fn test_should_overwrite_file_freshen_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("nonexistent.txt");
        let mut args = default_args();
        args.freshen = true;

        let decision = should_overwrite_file(&path, &args, None);
        assert_eq!(decision, OverwriteDecision::SkipQuietly);
    }

    #[test]
    fn test_should_overwrite_file_never_overwrite() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("existing.txt");
        fs::write(&path, "content").unwrap();

        let mut args = default_args();
        args.never_overwrite = true;
        args.overwrite = false;

        let decision = should_overwrite_file(&path, &args, None);
        assert_eq!(decision, OverwriteDecision::Skip);
    }

    #[test]
    fn test_should_overwrite_file_explicit_overwrite() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("existing.txt");
        fs::write(&path, "content").unwrap();

        let mut args = default_args();
        args.overwrite = true;

        let decision = should_overwrite_file(&path, &args, None);
        assert_eq!(decision, OverwriteDecision::Overwrite);
    }

    #[test]
    fn test_should_overwrite_file_default_existing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("existing.txt");
        fs::write(&path, "content").unwrap();

        let mut args = default_args();
        args.overwrite = false;

        let decision = should_overwrite_file(&path, &args, None);
        assert_eq!(decision, OverwriteDecision::Skip);
    }

    #[test]
    fn test_multiple_patterns() {
        let zip_data = create_test_zip(&[
            ("file.txt", b"Text"),
            ("file.rs", b"Rust"),
            ("file.md", b"Markdown"),
            ("file.json", b"JSON"),
        ]);

        let temp_dir = tempfile::tempdir().unwrap();
        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        let mut args = default_args();
        args.output_dir = Some(temp_dir.path().to_path_buf());
        args.patterns = vec!["*.txt".to_string(), "*.rs".to_string()];

        extract_archive(&mut archive, &args).unwrap();

        assert!(temp_dir.path().join("file.txt").exists());
        assert!(temp_dir.path().join("file.rs").exists());
        assert!(!temp_dir.path().join("file.md").exists());
        assert!(!temp_dir.path().join("file.json").exists());
    }
}
