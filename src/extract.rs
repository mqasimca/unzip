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
use memmap2::Mmap;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;
use zip::ZipArchive;

use crate::args::Args;
use crate::linux::{fadvise_dontneed, preallocate_file};
use crate::password::{get_password, is_password_error, prompt_for_password};
use crate::utils::{PatternMatcher, datetime_to_filetime, datetime_to_system_time, format_size};

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

pub enum ArchiveSource {
    FilePath(PathBuf),
    Mmap(Arc<Mmap>),
}

trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

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

fn open_archive_from_source(source: &ArchiveSource) -> Result<ZipArchive<Box<dyn ReadSeek + '_>>> {
    match source {
        ArchiveSource::FilePath(path) => {
            let file = File::open(path)
                .with_context(|| format!("Failed to open ZIP file: {}", path.display()))?;
            let file_size = file.metadata()?.len();
            crate::linux::fadvise_sequential(&file, file_size);
            let reader: Box<dyn ReadSeek> = Box::new(file);
            Ok(ZipArchive::new(reader)?)
        },
        ArchiveSource::Mmap(mmap) => {
            let cursor = std::io::Cursor::new(&mmap[..]);
            let reader: Box<dyn ReadSeek> = Box::new(cursor);
            Ok(ZipArchive::new(reader)?)
        },
    }
}

fn candidate_thread_count(args: &Args) -> usize {
    if args.quiet == 0 {
        return 1;
    }
    let auto = thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let requested = args.threads.unwrap_or(auto);
    if requested == 0 { 1 } else { requested }
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
    let mut stdout_lock = BufWriter::with_capacity(BUFFER_SIZE, stdout.lock());
    let mut buffer = vec![0u8; BUFFER_SIZE];

    let password = Mutex::new(get_password(args.password.as_deref(), args.quiet)?);
    let matcher = PatternMatcher::new(&args.patterns, &args.exclude, args.case_insensitive);
    let use_filters = !(args.patterns.is_empty() && args.exclude.is_empty());
    let exact_target = if args.patterns.len() == 1
        && args.exclude.is_empty()
        && !args.case_insensitive
    {
        let pattern = &args.patterns[0];
        if !pattern.contains('*') && !pattern.contains('?') {
            Some(pattern.as_str())
        } else {
            None
        }
    } else {
        None
    };

    for i in 0..archive.len() {
        let mut write_file =
            |file: &mut zip::read::ZipFile, name_for_msg: Option<&str>| -> Result<()> {
            loop {
                let bytes_read = match file.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) => {
                        let name = name_for_msg.unwrap_or_else(|| file.name());
                        bail!("Failed to read {}: {}", name, e);
                    },
                };
                if let Err(e) = stdout_lock.write_all(&buffer[..bytes_read]) {
                    let name = name_for_msg.unwrap_or_else(|| file.name());
                    bail!("Failed to write {} to stdout: {}", name, e);
                }
            }
            Ok(())
        };

        let mut decrypt_name: Option<String> = None;
        let needs_decrypt = {
            let file_result = archive.by_index(i);
            match file_result {
                Ok(mut file) => {
                    if file.is_dir() {
                        continue;
                    }

                    let name = file.name();
                    if let Some(target) = exact_target {
                        if name != target {
                            continue;
                        }
                    } else if use_filters && !matcher.should_extract(name) {
                        continue;
                    }

                    if file.encrypted() {
                        decrypt_name = Some(file.name().to_string());
                        true
                    } else {
                        write_file(&mut file, None)?;
                        continue;
                    }
                },
                Err(e) => {
                    let err_str = e.to_string();
                    if is_password_error(&err_str) {
                        true
                    } else {
                        bail!("Failed to read file: {}", err_str);
                    }
                },
            }
        };

        if !needs_decrypt {
            continue;
        }

        let decrypt_label = decrypt_name.as_deref();
        let mut pwd = password.lock().unwrap();
        if pwd.is_none() {
            if args.quiet == 0 {
                if let Some(name) = decrypt_label {
                    eprintln!("Encrypted file detected: {}", name);
                } else {
                    eprintln!("Encrypted file detected");
                }
            }
            *pwd = Some(prompt_for_password()?);
        }
        let pwd_bytes = pwd.clone();
        drop(pwd);

        if let Some(ref pwd) = pwd_bytes {
            let mut file = archive
                .by_index_decrypt(i, pwd)
                .with_context(|| {
                    if let Some(name) = decrypt_label {
                        format!("Failed to decrypt {}", name)
                    } else {
                        "Failed to decrypt file".to_string()
                    }
                })?;

            if file.is_dir() {
                continue;
            }

            let name = file.name();
            if let Some(target) = exact_target {
                if name != target {
                    continue;
                }
            } else if use_filters && !matcher.should_extract(name) {
                continue;
            }

            write_file(&mut file, None)?;
        } else {
            if let Some(name) = decrypt_label {
                bail!("Password required but not available for file: {}", name);
            } else {
                bail!("Password required but not available");
            }
        }
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
    extract_archive_serial(archive, args)
}

fn extract_archive_serial<R: Read + Seek>(archive: &mut ZipArchive<R>, args: &Args) -> Result<()> {
    let output_dir = args.output_dir.clone().unwrap_or_else(|| PathBuf::from("."));

    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).with_context(|| {
            format!("Failed to create output directory: {}", output_dir.display())
        })?;
    }

    let total_files = archive.len();
    let mut extracted = 0usize;
    let mut skipped = 0usize;
    let mut total_bytes = 0u64;

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

    // Track directories for timestamp restoration after extraction
    let mut directories: Vec<(PathBuf, Option<zip::DateTime>)> = Vec::new();
    let matcher = PatternMatcher::new(&args.patterns, &args.exclude, args.case_insensitive);

    let mut buffer = vec![0u8; BUFFER_SIZE];

    'main_loop: for i in 0..total_files {
        let result = archive.by_index(i);
        let mut file = if let Ok(f) = result {
            f
        } else {
            let err_str = result.as_ref().err().unwrap().to_string();
            let is_pwd_error = is_password_error(&err_str);
            drop(result);

            if is_pwd_error {
                let mut pwd = password.lock().unwrap();
                if pwd.is_none() {
                    if args.quiet == 0 {
                        if let Some(ref pb) = progress_bar {
                            pb.println("Encrypted file detected".to_string());
                        } else {
                            eprintln!("Encrypted file detected");
                        }
                    }
                    *pwd = Some(prompt_for_password()?);
                }
                let pwd_bytes = pwd.clone();
                drop(pwd);

                if let Some(ref pwd) = pwd_bytes {
                    match archive.by_index_decrypt(i, pwd) {
                        Ok(f) => f,
                        Err(_e) => {
                            if args.quiet < 2 {
                                if let Some(ref pb) = progress_bar {
                                    pb.println("    error: Invalid password".to_string());
                                } else {
                                    eprintln!("error: Invalid password");
                                }
                            }
                            if let Some(ref pb) = progress_bar {
                                pb.inc(1);
                            }
                            skipped += 1;
                            continue 'main_loop;
                        },
                    }
                } else {
                    if args.quiet < 2 {
                        if let Some(ref pb) = progress_bar {
                            pb.println("    error: Password required".to_string());
                        } else {
                            eprintln!("error: Password required");
                        }
                    }
                    if let Some(ref pb) = progress_bar {
                        pb.inc(1);
                    }
                    skipped += 1;
                    continue 'main_loop;
                }
            } else {
                bail!("Failed to read file: {}", err_str);
            }
        };

        let name = file.name().to_string();
        let mtime = file.last_modified();
        let size = file.size();
        let is_dir = file.is_dir();

        if is_dir {
            if !args.junk_paths {
                let dir_name = if args.lowercase {
                    name.to_lowercase()
                } else {
                    name.clone()
                };
                let outpath = output_dir.join(&dir_name);
                fs::create_dir_all(&outpath)
                    .with_context(|| format!("Failed to create directory: {}", outpath.display()))?;
                directories.push((outpath, mtime));
            }
            if let Some(ref pb) = progress_bar {
                pb.inc(1);
            }
            continue;
        }

        if !matcher.should_extract(&name) {
            if let Some(ref pb) = progress_bar {
                pb.inc(1);
            }
            skipped += 1;
            continue;
        }

        let outpath = if args.junk_paths {
            let filename = std::path::Path::new(&name)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| name.clone());
            let filename = if args.lowercase {
                filename.to_lowercase()
            } else {
                filename
            };
            output_dir.join(filename)
        } else {
            let name_out = if args.lowercase {
                name.to_lowercase()
            } else {
                name.clone()
            };
            match file.enclosed_name() {
                Some(_) => output_dir.join(&name_out),
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
                skipped += 1;
                continue;
            },
            OverwriteDecision::SkipQuietly => {
                if let Some(ref pb) = progress_bar {
                    pb.inc(1);
                }
                skipped += 1;
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

        extracted += 1;
        total_bytes += size;

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

    if args.quiet == 0 {
        println!(
            "Extracted {} files ({}) to {}",
            extracted,
            format_size(total_bytes),
            output_dir.display()
        );
        if skipped > 0 {
            println!("Skipped {} files", skipped);
        }
    }

    Ok(())
}

pub fn extract_archive_threaded(source: ArchiveSource, args: &Args) -> Result<()> {
    let output_dir = args.output_dir.clone().unwrap_or_else(|| PathBuf::from("."));

    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).with_context(|| {
            format!("Failed to create output directory: {}", output_dir.display())
        })?;
    }

    let mut candidate_threads = candidate_thread_count(args);
    if candidate_threads <= 1 {
        let mut archive = open_archive_from_source(&source)?;
        return extract_archive_serial(&mut archive, args);
    }

    let matcher = PatternMatcher::new(&args.patterns, &args.exclude, args.case_insensitive);
    let password_bytes = get_password(args.password.as_deref(), args.quiet)?;
    let mut archive = open_archive_from_source(&source)?;
    let total_files = archive.len();
    let mut directories: Vec<(PathBuf, Option<zip::DateTime>)> = Vec::new();
    let mut jobs: Vec<FileJob> = Vec::new();
    let mut skipped = 0usize;
    let mut encrypted_found = false;

    for i in 0..total_files {
        let file = archive.by_index(i)?;
        let name = file.name().to_string();
        let is_dir = file.is_dir();
        let mtime = file.last_modified();
        let size = file.size();
        let encrypted = file.encrypted();

        if is_dir {
            if !args.junk_paths {
                let dir_name = if args.lowercase {
                    name.to_lowercase()
                } else {
                    name.clone()
                };
                directories.push((output_dir.join(dir_name), mtime));
            }
            continue;
        }

        if !matcher.should_extract(&name) {
            skipped += 1;
            continue;
        }

        if encrypted {
            encrypted_found = true;
        }

        jobs.push(FileJob {
            index: i,
            name,
            size,
            mtime,
            encrypted,
        });
    }

    if encrypted_found && password_bytes.is_none() {
        let mut archive = open_archive_from_source(&source)?;
        return extract_archive_serial(&mut archive, args);
    }

    if jobs.is_empty() {
        for (dir_path, _) in &directories {
            fs::create_dir_all(dir_path)
                .with_context(|| format!("Failed to create directory: {}", dir_path.display()))?;
        }
        if !args.no_timestamps {
            for (dir_path, mtime) in directories.iter().rev() {
                if let Some(dt) = mtime {
                    let filetime_mtime = datetime_to_filetime(*dt);
                    filetime::set_file_mtime(dir_path, filetime_mtime).ok();
                }
            }
        }
        return Ok(());
    }

    if candidate_threads > jobs.len() {
        candidate_threads = jobs.len();
    }

    for (dir_path, _) in &directories {
        fs::create_dir_all(dir_path)
            .with_context(|| format!("Failed to create directory: {}", dir_path.display()))?;
    }

    drop(archive);

    let extracted = Arc::new(AtomicUsize::new(0));
    let skipped_files = Arc::new(AtomicUsize::new(skipped));
    let total_bytes = Arc::new(AtomicU64::new(0));
    let source = Arc::new(source);
    let output_dir = Arc::new(output_dir);
    let password = Arc::new(password_bytes);
    let args = Arc::new(args.clone());

    let chunk_size = (jobs.len() + candidate_threads - 1) / candidate_threads;
    let mut handles = Vec::with_capacity(candidate_threads);

    for chunk in jobs.chunks(chunk_size) {
        let chunk = chunk.to_vec();
        let source = Arc::clone(&source);
        let output_dir = Arc::clone(&output_dir);
        let args = Arc::clone(&args);
        let password = Arc::clone(&password);

        let extracted_ref = Arc::clone(&extracted);
        let skipped_ref = Arc::clone(&skipped_files);
        let bytes_ref = Arc::clone(&total_bytes);

        handles.push(thread::spawn(move || -> Result<()> {
            let mut archive = open_archive_from_source(&source)?;
            let mut buffer = vec![0u8; BUFFER_SIZE];

            for job in chunk {
                let mut file = if job.encrypted {
                    let pwd = password.as_ref().as_ref().ok_or_else(|| {
                        anyhow::anyhow!("Password required for encrypted file")
                    })?;
                    archive.by_index_decrypt(job.index, pwd)?
                } else {
                    archive.by_index(job.index)?
                };

                let outpath = if args.junk_paths {
                    let filename = std::path::Path::new(&job.name)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| job.name.clone());
                    let filename = if args.lowercase {
                        filename.to_lowercase()
                    } else {
                        filename
                    };
                    output_dir.join(filename)
                } else {
                    let name_out = if args.lowercase {
                        job.name.to_lowercase()
                    } else {
                        job.name.clone()
                    };
                    match file.enclosed_name() {
                        Some(_) => output_dir.join(&name_out),
                        None => {
                            skipped_ref.fetch_add(1, Ordering::Relaxed);
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

                let decision = should_overwrite_file(&outpath, &args, job.mtime);

                match decision {
                    OverwriteDecision::Skip | OverwriteDecision::SkipQuietly => {
                        skipped_ref.fetch_add(1, Ordering::Relaxed);
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
                finalize_extracted_file(&outpath, job.mtime, unix_mode, args.no_timestamps);

                extracted_ref.fetch_add(1, Ordering::Relaxed);
                bytes_ref.fetch_add(job.size, Ordering::Relaxed);
            }

            Ok(())
        }));
    }

    for handle in handles {
        handle.join().expect("thread panicked")?;
    }

    if !args.no_timestamps {
        for (dir_path, mtime) in directories.iter().rev() {
            if let Some(dt) = mtime {
                let filetime_mtime = datetime_to_filetime(*dt);
                filetime::set_file_mtime(dir_path, filetime_mtime).ok();
            }
        }
    }

    if args.quiet == 0 {
        let extract_count = extracted.load(Ordering::Relaxed);
        let skip_count = skipped_files.load(Ordering::Relaxed);
        let bytes = total_bytes.load(Ordering::Relaxed);
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

#[derive(Clone)]
struct FileJob {
    index: usize,
    name: String,
    size: u64,
    mtime: Option<zip::DateTime>,
    encrypted: bool,
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
    fn test_zip_extract_threaded() {
        let zip_data = create_test_zip(&[
            ("test.txt", b"Test content"),
            ("subdir/nested.txt", b"Nested content"),
        ]);

        let temp_dir = tempfile::tempdir().unwrap();
        let zip_path = temp_dir.path().join("test.zip");
        fs::write(&zip_path, zip_data).unwrap();

        let output_dir = temp_dir.path().join("out");
        let mut args = default_args();
        args.output_dir = Some(output_dir.clone());
        args.quiet = 2;
        args.threads = Some(2);

        extract_archive_threaded(ArchiveSource::FilePath(zip_path), &args).unwrap();

        let test_file = output_dir.join("test.txt");
        assert!(test_file.exists());
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "Test content");

        let nested_file = output_dir.join("subdir/nested.txt");
        assert!(nested_file.exists());
        assert_eq!(fs::read_to_string(&nested_file).unwrap(), "Nested content");
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
