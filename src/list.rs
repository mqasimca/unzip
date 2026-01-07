//! Archive listing functionality
//!
//! This module provides functions to list ZIP archive contents without extracting files.
//! Supports both short format (filenames only) and verbose format (with sizes, dates,
//! compression ratios, and CRC values).
//!
//! # Output Formats
//!
//! **Short format** (`-l`): Simple filename listing with basic metadata
//! **Verbose format** (`-v`): Detailed listing including:
//! - File sizes (uncompressed and compressed)
//! - Compression ratios
//! - Last modification dates and times
//! - CRC32 checksums
//! - File attributes and permissions
//!
//! # Examples
//!
//! ```no_run
//! use std::fs::File;
//! use zip::ZipArchive;
//! use unzip::list_contents;
//!
//! let file = File::open("archive.zip")?;
//! let mut archive = ZipArchive::new(file)?;
//! list_contents(&mut archive, false)?;  // Short format
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use anyhow::Result;
use std::io::{Read, Seek};
use zip::ZipArchive;

use crate::utils::{format_datetime, format_size};

/// Display the ZIP archive comment if present.
///
/// Prints the archive comment to stdout. If the archive has no comment,
/// this function does nothing.
///
/// # Arguments
///
/// * `archive` - The ZIP archive to read the comment from
///
/// # Errors
///
/// Returns an error if the archive metadata cannot be read (though this is rare).
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use zip::ZipArchive;
/// use unzip::display_comment;
///
/// let file = File::open("archive.zip")?;
/// let mut archive = ZipArchive::new(file)?;
/// display_comment(&mut archive)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn display_comment<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<()> {
    let comment = archive.comment();
    if !comment.is_empty() {
        println!("{}", String::from_utf8_lossy(comment));
    }
    Ok(())
}

/// List the contents of a ZIP archive in short or verbose format.
///
/// Displays information about all files in the archive without extracting them.
/// The output format depends on the verbose flag:
///
/// - **Short format** (`verbose = false`): Shows file sizes, modification dates, and names
/// - **Verbose format** (`verbose = true`): Shows uncompressed size, compressed size,
///   compression ratio, date/time, CRC32 checksum, and name
///
/// # Arguments
///
/// * `archive` - The ZIP archive to list
/// * `verbose` - If true, use verbose format with detailed file information
///
/// # Errors
///
/// Returns an error if:
/// - Archive files cannot be read
/// - File metadata is corrupted or invalid
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use zip::ZipArchive;
/// use unzip::list_contents;
///
/// let file = File::open("archive.zip")?;
/// let mut archive = ZipArchive::new(file)?;
///
/// // Short format
/// list_contents(&mut archive, false)?;
///
/// // Verbose format
/// list_contents(&mut archive, true)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn list_contents<R: Read + Seek>(archive: &mut ZipArchive<R>, verbose: bool) -> Result<()> {
    if verbose {
        println!(
            "{:>8}  {:>8}  {:>5}  {:>19}  {:>8}  Name",
            "Length", "Size", "Ratio", "Date & Time", "CRC-32"
        );
        println!("{}", "-".repeat(80));
    } else {
        println!("{:>10}  {:>19}  Name", "Size", "Modified");
        println!("{:->10}  {:->19}  {:->40}", "", "", "");
    }

    let mut total_size: u64 = 0;
    let mut total_compressed: u64 = 0;
    let mut file_count = 0;

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let size = file.size();
        let compressed = file.compressed_size();
        total_size += size;
        total_compressed += compressed;
        file_count += 1;

        let datetime_str = format_datetime(file.last_modified());
        let name = file.name();

        if verbose {
            let ratio = if size > 0 {
                100 - (compressed * 100 / size)
            } else {
                0
            };
            let crc = file.crc32();
            println!(
                "{:>8}  {:>8}  {:>4}%  {}  {:08x}  {}",
                size, compressed, ratio, datetime_str, crc, name
            );
        } else {
            println!("{:>10}  {}  {}", format_size(size), datetime_str, name);
        }
    }

    if verbose {
        println!("{}", "-".repeat(80));
        let ratio = if total_size > 0 {
            100 - (total_compressed * 100 / total_size)
        } else {
            0
        };
        println!(
            "{:>8}  {:>8}  {:>4}%  {:>19}  {:>8}  {} files",
            total_size, total_compressed, ratio, "", "", file_count
        );
    } else {
        println!("{:->10}  {:->19}  {:->40}", "", "", "");
        println!("{:>10}  {:>19}  {} files", format_size(total_size), "", file_count);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
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

    fn create_test_zip_with_comment(files: &[(&str, &[u8])], comment: &str) -> Vec<u8> {
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
            zip.set_comment(comment);
            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_list_contents_short_format() {
        let zip_data =
            create_test_zip(&[("test.txt", b"Test content"), ("file.rs", b"Rust file content")]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        // Should not panic and should return Ok
        let result = list_contents(&mut archive, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_contents_verbose_format() {
        let zip_data =
            create_test_zip(&[("test.txt", b"Test content"), ("file.rs", b"Rust file content")]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        // Should not panic and should return Ok
        let result = list_contents(&mut archive, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_contents_empty_archive() {
        let zip_data = create_test_zip(&[]);

        let cursor = Cursor::new(zip_data.clone());
        let mut archive = ZipArchive::new(cursor).unwrap();

        assert_eq!(archive.len(), 0);

        // Should handle empty archives gracefully
        let result = list_contents(&mut archive, false);
        assert!(result.is_ok());

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();
        let result = list_contents(&mut archive, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_contents_with_directories() {
        let zip_data = create_test_zip(&[
            ("dir1/", &[]),
            ("dir1/file.txt", b"Content"),
            ("dir2/", &[]),
            ("dir2/nested/", &[]),
            ("dir2/nested/deep.txt", b"Deep content"),
        ]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        // Should handle directories correctly
        let result = list_contents(&mut archive, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_contents_large_files() {
        let large_content = vec![b'A'; 10 * 1024 * 1024]; // 10MB

        let zip_data = create_test_zip(&[("small.txt", b"small"), ("large.bin", &large_content)]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        // Should handle large files correctly
        let result = list_contents(&mut archive, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_contents_unicode_filenames() {
        let zip_data = create_test_zip(&[
            ("test.txt", b"English"),
            ("テスト.txt", b"Japanese"),
            ("测试.txt", b"Chinese"),
            ("тест.txt", b"Russian"),
            ("🎉emoji.txt", b"Emoji"),
        ]);

        let cursor = Cursor::new(zip_data.clone());
        let mut archive = ZipArchive::new(cursor).unwrap();

        // Should handle Unicode filenames correctly
        let result = list_contents(&mut archive, false);
        assert!(result.is_ok());

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();
        let result = list_contents(&mut archive, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_display_comment_with_comment() {
        let zip_data = create_test_zip_with_comment(
            &[("test.txt", b"Content")],
            "This is a test archive comment",
        );

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        // Should display comment without error
        let result = display_comment(&mut archive);
        assert!(result.is_ok());
    }

    #[test]
    fn test_display_comment_without_comment() {
        let zip_data = create_test_zip(&[("test.txt", b"Content")]);

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        // Should handle missing comment gracefully
        let result = display_comment(&mut archive);
        assert!(result.is_ok());
    }

    #[test]
    fn test_display_comment_empty_comment() {
        let zip_data = create_test_zip_with_comment(&[("test.txt", b"Content")], "");

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        // Should handle empty comment gracefully
        let result = display_comment(&mut archive);
        assert!(result.is_ok());
    }

    #[test]
    fn test_display_comment_multiline() {
        let zip_data =
            create_test_zip_with_comment(&[("test.txt", b"Content")], "Line 1\nLine 2\nLine 3");

        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor).unwrap();

        // Should handle multiline comments
        let result = display_comment(&mut archive);
        assert!(result.is_ok());
    }
}
