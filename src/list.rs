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
use std::io::{Read, Seek, Write};
use zip::ZipArchive;

struct DateTimeCache {
    last: Option<zip::DateTime>,
    buf: [u8; 19],
}

impl DateTimeCache {
    fn new() -> Self {
        Self { last: None, buf: [b' '; 19] }
    }

    fn as_str(&mut self, datetime: Option<zip::DateTime>) -> &str {
        match datetime {
            Some(dt) => {
                if self.last != Some(dt) {
                    let (y, m, d, h, min, s) =
                        (dt.year(), dt.month(), dt.day(), dt.hour(), dt.minute(), dt.second());
                    self.buf[0] = b'0' + (y / 1000 % 10) as u8;
                    self.buf[1] = b'0' + (y / 100 % 10) as u8;
                    self.buf[2] = b'0' + (y / 10 % 10) as u8;
                    self.buf[3] = b'0' + (y % 10) as u8;
                    self.buf[4] = b'-';
                    self.buf[5] = b'0' + (m / 10 % 10) as u8;
                    self.buf[6] = b'0' + (m % 10) as u8;
                    self.buf[7] = b'-';
                    self.buf[8] = b'0' + (d / 10 % 10) as u8;
                    self.buf[9] = b'0' + (d % 10) as u8;
                    self.buf[10] = b' ';
                    self.buf[11] = b'0' + (h / 10 % 10) as u8;
                    self.buf[12] = b'0' + (h % 10) as u8;
                    self.buf[13] = b':';
                    self.buf[14] = b'0' + (min / 10 % 10) as u8;
                    self.buf[15] = b'0' + (min % 10) as u8;
                    self.buf[16] = b':';
                    self.buf[17] = b'0' + (s / 10 % 10) as u8;
                    self.buf[18] = b'0' + (s % 10) as u8;
                    self.last = Some(dt);
                }
                unsafe { std::str::from_utf8_unchecked(&self.buf) }
            },
            None => "                   ",
        }
    }
}

fn write_u64(buf: &mut [u8; 32], mut value: u64) -> usize {
    let mut tmp = [0u8; 20];
    let mut idx = 0;
    if value == 0 {
        tmp[idx] = b'0';
        idx += 1;
    } else {
        while value > 0 {
            tmp[idx] = b'0' + (value % 10) as u8;
            value /= 10;
            idx += 1;
        }
    }
    for i in 0..idx {
        buf[i] = tmp[idx - 1 - i];
    }
    idx
}

fn write_hex_u32(buf: &mut [u8; 8], value: u32) {
    let mut v = value;
    for i in (0..8).rev() {
        let digit = (v & 0xF) as u8;
        buf[i] = match digit {
            0..=9 => b'0' + digit,
            _ => b'a' + (digit - 10),
        };
        v >>= 4;
    }
}

fn size_to_str<'a>(buf: &'a mut [u8; 32], size: u64) -> &'a str {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    let mut pos = 0;
    if size >= GB {
        let scaled = size * 10 / GB;
        pos += write_u64(buf, scaled / 10);
        buf[pos] = b'.';
        pos += 1;
        buf[pos] = b'0' + (scaled % 10) as u8;
        pos += 1;
        buf[pos] = b'G';
        pos += 1;
    } else if size >= MB {
        let scaled = size * 10 / MB;
        pos += write_u64(buf, scaled / 10);
        buf[pos] = b'.';
        pos += 1;
        buf[pos] = b'0' + (scaled % 10) as u8;
        pos += 1;
        buf[pos] = b'M';
        pos += 1;
    } else if size >= KB {
        let scaled = size * 10 / KB;
        pos += write_u64(buf, scaled / 10);
        buf[pos] = b'.';
        pos += 1;
        buf[pos] = b'0' + (scaled % 10) as u8;
        pos += 1;
        buf[pos] = b'K';
        pos += 1;
    } else {
        pos += write_u64(buf, size);
        buf[pos] = b'B';
        pos += 1;
    }

    unsafe { std::str::from_utf8_unchecked(&buf[..pos]) }
}

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
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let comment = archive.comment();
    if !comment.is_empty() {
        writeln!(&mut out, "{}", String::from_utf8_lossy(comment))?;
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
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let mut datetime_cache = DateTimeCache::new();
    let mut size_buf = [0u8; 32];
    let mut num_buf = [0u8; 32];
    let mut crc_buf = [0u8; 8];
    if verbose {
        writeln!(
            &mut out,
            "{:>8}  {:>8}  {:>5}  {:>19}  {:>8}  Name",
            "Length", "Size", "Ratio", "Date & Time", "CRC-32"
        )?;
        writeln!(&mut out, "{}", "-".repeat(80))?;
    } else {
        writeln!(&mut out, "{:>10}  {:>19}  Name", "Size", "Modified")?;
        writeln!(&mut out, "{:->10}  {:->19}  {:->40}", "", "", "")?;
    }

    let mut total_size: u64 = 0;
    let mut total_compressed: u64 = 0;
    let mut file_count = 0;

    // Pre-allocate line buffer to avoid allocations per file
    let mut line_buf = Vec::with_capacity(512);

    for i in 0..archive.len() {
        let file = archive.by_index_raw(i)?;
        let size = file.size();
        let compressed = file.compressed_size();
        total_size += size;
        total_compressed += compressed;
        file_count += 1;

        let datetime_str = datetime_cache.as_str(file.last_modified());
        let name = file.name();

        line_buf.clear();

        if verbose {
            let ratio = if size > 0 {
                100 - (compressed * 100 / size)
            } else {
                0
            };

            // Build complete line in buffer with single write
            // Right-align size (8 chars)
            let size_len = write_u64(&mut num_buf, size);
            for _ in 0..(8_usize.saturating_sub(size_len)) {
                line_buf.push(b' ');
            }
            line_buf.extend_from_slice(&num_buf[..size_len]);
            line_buf.extend_from_slice(b"  ");

            // Right-align compressed size (8 chars)
            let comp_len = write_u64(&mut num_buf, compressed);
            for _ in 0..(8_usize.saturating_sub(comp_len)) {
                line_buf.push(b' ');
            }
            line_buf.extend_from_slice(&num_buf[..comp_len]);
            line_buf.extend_from_slice(b"  ");

            // Right-align ratio (4 chars)
            let ratio_len = write_u64(&mut num_buf, ratio as u64);
            for _ in 0..(4_usize.saturating_sub(ratio_len)) {
                line_buf.push(b' ');
            }
            line_buf.extend_from_slice(&num_buf[..ratio_len]);
            line_buf.extend_from_slice(b"%  ");

            line_buf.extend_from_slice(datetime_str.as_bytes());
            line_buf.extend_from_slice(b"  ");

            write_hex_u32(&mut crc_buf, file.crc32());
            line_buf.extend_from_slice(&crc_buf);
            line_buf.extend_from_slice(b"  ");

            line_buf.extend_from_slice(name.as_bytes());
            line_buf.push(b'\n');

            // Single write for entire line
            out.write_all(&line_buf)?;
        } else {
            let size_str = size_to_str(&mut size_buf, size);

            // Right-align size (10 chars)
            for _ in 0..(10_usize.saturating_sub(size_str.len())) {
                line_buf.push(b' ');
            }
            line_buf.extend_from_slice(size_str.as_bytes());
            line_buf.extend_from_slice(b"  ");
            line_buf.extend_from_slice(datetime_str.as_bytes());
            line_buf.extend_from_slice(b"  ");
            line_buf.extend_from_slice(name.as_bytes());
            line_buf.push(b'\n');

            // Single write for entire line
            out.write_all(&line_buf)?;
        }
    }

    if verbose {
        writeln!(&mut out, "{}", "-".repeat(80))?;
        let ratio = if total_size > 0 {
            100 - (total_compressed * 100 / total_size)
        } else {
            0
        };
        writeln!(
            &mut out,
            "{:>8}  {:>8}  {:>4}%  {:>19}  {:>8}  {} files",
            total_size, total_compressed, ratio, "", "", file_count
        )?;
    } else {
        writeln!(&mut out, "{:->10}  {:->19}  {:->40}", "", "", "")?;
        let total_str = size_to_str(&mut size_buf, total_size);

        // Build footer line in buffer with single write
        line_buf.clear();
        for _ in 0..(10_usize.saturating_sub(total_str.len())) {
            line_buf.push(b' ');
        }
        line_buf.extend_from_slice(total_str.as_bytes());
        line_buf.extend_from_slice(b"  ");
        line_buf.extend_from_slice(b"                   ");
        line_buf.extend_from_slice(b"  ");

        let count_len = write_u64(&mut num_buf, file_count as u64);
        line_buf.extend_from_slice(&num_buf[..count_len]);
        line_buf.extend_from_slice(b" files\n");

        out.write_all(&line_buf)?;
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
