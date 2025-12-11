//! Archive extraction functionality

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use zip::ZipArchive;

use crate::args::Args;
use crate::linux::{fadvise_dontneed, preallocate_file};
use crate::utils::{datetime_to_filetime, datetime_to_system_time, format_size, should_extract};

/// Buffer size for file I/O (256KB for better throughput)
const BUFFER_SIZE: usize = 256 * 1024;

/// Extract files to stdout/pipe
pub fn extract_to_pipe<R: Read + Seek>(archive: &mut ZipArchive<R>, args: &Args) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if file.is_dir() {
            continue;
        }

        // Check if file matches patterns
        if !should_extract(&name, &args.patterns, &args.exclude, args.case_insensitive) {
            continue;
        }

        io::copy(&mut file, &mut stdout_lock)
            .with_context(|| format!("Failed to write {} to stdout", name))?;
    }

    Ok(())
}

/// Extract archive to filesystem with Linux optimizations
pub fn extract_archive<R: Read + Seek>(archive: &mut ZipArchive<R>, args: &Args) -> Result<()> {
    let output_dir = args
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir)
            .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;
    }

    let total_files = archive.len();
    let extracted = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let total_bytes = AtomicU64::new(0);

    let progress_bar = if args.quiet == 0 {
        let pb = ProgressBar::new(total_files as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    // Collect file info
    let mut file_infos: Vec<(usize, String, bool, u64, Option<zip::DateTime>)> = Vec::new();

    for i in 0..total_files {
        let file = archive.by_index(i)?;
        let name = file.name().to_string();
        let is_dir = file.is_dir();
        let size = file.size();
        let mtime = file.last_modified();
        file_infos.push((i, name, is_dir, size, mtime));
    }

    // First pass: create all directories (must be sequential)
    for (_, name, is_dir, _, _) in &file_infos {
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
    }

    // Pre-allocate buffer for extraction
    let mut buffer = vec![0u8; BUFFER_SIZE];

    // Second pass: extract files
    for (i, name, is_dir, size, mtime) in file_infos {
        if is_dir {
            if let Some(ref pb) = progress_bar {
                pb.inc(1);
            }
            continue;
        }

        // Check if file matches patterns
        if !should_extract(&name, &args.patterns, &args.exclude, args.case_insensitive) {
            if let Some(ref pb) = progress_bar {
                pb.inc(1);
            }
            skipped.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        let mut file = archive.by_index(i)?;

        let outpath = if args.junk_paths {
            // Extract filename only, no path
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
                }
            }
        };

        // Create parent directories if needed
        if let Some(parent) = outpath.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
            }
        }

        // Handle freshen/update modes
        if args.freshen || args.update {
            if outpath.exists() {
                // Check if archive file is newer
                if let Ok(meta) = outpath.metadata() {
                    if let Ok(disk_mtime) = meta.modified() {
                        if let Some(archive_mtime) = mtime {
                            let archive_time = datetime_to_system_time(archive_mtime);
                            if archive_time <= disk_mtime {
                                // Archive file is not newer, skip
                                if let Some(ref pb) = progress_bar {
                                    pb.inc(1);
                                }
                                skipped.fetch_add(1, Ordering::Relaxed);
                                continue;
                            }
                        }
                    }
                }
            } else if args.freshen {
                // Freshen mode: don't create new files
                if let Some(ref pb) = progress_bar {
                    pb.inc(1);
                }
                skipped.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        }

        // Handle overwrite logic
        if outpath.exists() {
            if args.never_overwrite {
                if args.quiet == 0 {
                    if let Some(ref pb) = progress_bar {
                        pb.println(format!("    skipping: {} (already exists)", name));
                    }
                }
                if let Some(ref pb) = progress_bar {
                    pb.inc(1);
                }
                skipped.fetch_add(1, Ordering::Relaxed);
                continue;
            } else if !args.overwrite && !args.freshen && !args.update {
                if args.quiet == 0 {
                    if let Some(ref pb) = progress_bar {
                        pb.println(format!("    skipping: {} (use -o to overwrite)", name));
                    }
                }
                if let Some(ref pb) = progress_bar {
                    pb.inc(1);
                }
                skipped.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        }

        // Create output file
        let outfile = File::create(&outpath)
            .with_context(|| format!("Failed to create file: {}", outpath.display()))?;

        // Linux optimization: pre-allocate disk space to avoid fragmentation
        if size > 0 {
            preallocate_file(&outfile, size).ok();
        }

        // Use larger buffer for better throughput
        let mut writer = BufWriter::with_capacity(BUFFER_SIZE, outfile);

        // Manual copy with reused buffer for less allocation
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            writer.write_all(&buffer[..bytes_read])?;
        }

        let inner_file = writer.into_inner()?;

        // Linux optimization: tell kernel we're done with this file's cache
        fadvise_dontneed(&inner_file, 0, size);

        drop(inner_file);

        // Set file modification time
        if let Some(dt) = mtime {
            let mtime = datetime_to_filetime(dt);
            filetime::set_file_mtime(&outpath, mtime).ok();
        }

        // Set permissions on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode)).ok();
            }
        }

        if args.quiet == 0 {
            if let Some(ref pb) = progress_bar {
                pb.println(format!("  extracting: {}", name));
            }
        }

        extracted.fetch_add(1, Ordering::Relaxed);
        total_bytes.fetch_add(size, Ordering::Relaxed);

        if let Some(ref pb) = progress_bar {
            pb.inc(1);
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
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

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
            overwrite: true,
            never_overwrite: false,
            freshen: false,
            update: false,
            junk_paths: false,
            case_insensitive: false,
            lowercase: false,
            quiet: 2,
            threads: None,
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

        assert_eq!(
            fs::read_to_string(&existing_file).unwrap(),
            "Original content"
        );
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
