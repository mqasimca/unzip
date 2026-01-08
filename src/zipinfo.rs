//! Detailed ZIP archive information (zipinfo mode)
//!
//! Provides Info-ZIP compatible zipinfo functionality with various output formats
//! for detailed inspection of ZIP archives. Displays technical information including
//! file permissions, encryption status, compression methods, and file attributes.
//!
//! # Output Formats
//!
//! - **Short** (`-s`, default): Unix ls -l style with compression method
//! - **Medium** (`-m`): Short format plus compression percentage
//! - **Long** (`-l`): Short format plus compressed size in bytes
//! - **Verbose** (`-v`): Multi-page detailed format
//! - **Filenames only** (`-1`): One filename per line, no headers
//! - **Filenames with headers** (`-2`): Filenames with optional headers/trailers
//!
//! # Examples
//!
//! ```no_run
//! use std::fs::File;
//! use zip::ZipArchive;
//! use unzip::{Args, zipinfo::display_zipinfo};
//! use clap::Parser;
//!
//! let file = File::open("archive.zip")?;
//! let mut archive = ZipArchive::new(file)?;
//! let args = Args::parse();
//! display_zipinfo(&mut archive, &args)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use anyhow::Result;
use std::io::{Read, Seek, Write};
use zip::ZipArchive;

use crate::args::Args;
use crate::utils::PatternMatcher;

struct DateTimeCache {
    last: Option<zip::DateTime>,
    buf: [u8; 19],
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

fn write_right_aligned(out: &mut dyn Write, s: &str, width: usize) -> Result<()> {
    let len = s.len();
    if len < width {
        for _ in 0..(width - len) {
            out.write_all(b" ")?;
        }
    }
    out.write_all(s.as_bytes())?;
    Ok(())
}

impl DateTimeCache {
    fn new() -> Self {
        Self {
            last: None,
            buf: [b' '; 19],
        }
    }

    fn as_str(&mut self, datetime: Option<zip::DateTime>) -> &str {
        match datetime {
            Some(dt) => {
                if self.last != Some(dt) {
                    let (y, m, d, h, min, s) = (
                        dt.year(),
                        dt.month(),
                        dt.day(),
                        dt.hour(),
                        dt.minute(),
                        dt.second(),
                    );
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

/// Zipinfo output mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZipinfoMode {
    /// Filenames only, one per line (no headers)
    FilenamesOnly,
    /// Filenames only, but allow headers/trailers
    FilenamesWithHeaders,
    /// Short Unix ls -l format (default)
    Short,
    /// Medium format with compression percentage
    Medium,
    /// Long format with compressed size
    Long,
    /// Verbose multi-page format
    Verbose,
}

impl ZipinfoMode {
    /// Parse mode from command-line option
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "-1" | "1" => Some(Self::FilenamesOnly),
            "-2" | "2" => Some(Self::FilenamesWithHeaders),
            "-s" | "s" => Some(Self::Short),
            "-m" | "m" => Some(Self::Medium),
            "-l" | "l" => Some(Self::Long),
            "-v" | "v" => Some(Self::Verbose),
            _ => None,
        }
    }
}

/// Display zipinfo output for the archive
///
/// Shows detailed technical information about files in the ZIP archive in
/// various formats compatible with Info-ZIP's zipinfo utility.
///
/// # Arguments
///
/// * `archive` - The ZIP archive to inspect
/// * `args` - Command-line arguments with zipinfo mode and filters
///
/// # Errors
///
/// Returns an error if files cannot be read from the archive
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use zip::ZipArchive;
/// use unzip::{Args, zipinfo::display_zipinfo};
/// use clap::Parser;
///
/// let file = File::open("archive.zip")?;
/// let mut archive = ZipArchive::new(file)?;
/// let args = Args::parse();
/// display_zipinfo(&mut archive, &args)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn display_zipinfo<R: Read + Seek>(archive: &mut ZipArchive<R>, args: &Args) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let matcher = PatternMatcher::new(&args.patterns, &args.exclude, args.case_insensitive);
    let use_filters = !(args.patterns.is_empty() && args.exclude.is_empty());
    let mut datetime_cache = DateTimeCache::new();
    // Determine mode from zipinfo argument
    let mode = if let Some(Some(mode_str)) = &args.zipinfo {
        ZipinfoMode::from_str(mode_str).unwrap_or(ZipinfoMode::Short)
    } else {
        ZipinfoMode::Short // Default mode
    };

    // Print header (except for FilenamesOnly mode)
    if mode != ZipinfoMode::FilenamesOnly && args.quiet == 0 {
        print_header(&mut out, archive, args, &matcher, use_filters)?;
    }

    // Print file entries
    for i in 0..archive.len() {
        let file = archive.by_index_raw(i)?;
        let name = file.name();

        if use_filters && !matcher.should_extract(name) {
            continue;
        }

        match mode {
            ZipinfoMode::FilenamesOnly | ZipinfoMode::FilenamesWithHeaders => {
                writeln!(&mut out, "{}", name)?;
            },
            ZipinfoMode::Short => {
                print_short_format(&mut out, &file, name, &mut datetime_cache)?;
            },
            ZipinfoMode::Medium => {
                print_medium_format(&mut out, &file, name, &mut datetime_cache)?;
            },
            ZipinfoMode::Long => {
                print_long_format(&mut out, &file, name, &mut datetime_cache)?;
            },
            ZipinfoMode::Verbose => {
                print_verbose_format(&mut out, &file, name, &mut datetime_cache)?;
            },
        }
    }

    // Print trailer (except for FilenamesOnly mode)
    if mode != ZipinfoMode::FilenamesOnly && args.quiet == 0 {
        print_trailer(&mut out, archive, args)?;
    }

    Ok(())
}

/// Print archive header with summary information
fn print_header<R: Read + Seek>(
    out: &mut dyn Write,
    archive: &mut ZipArchive<R>,
    args: &Args,
    matcher: &PatternMatcher,
    use_filters: bool,
) -> Result<()> {
    let mut total_size: u64 = 0;
    let mut file_count: usize = 0;
    for i in 0..archive.len() {
        if let Ok(f) = archive.by_index_raw(i) {
            let name = f.name();
            if !use_filters || matcher.should_extract(name) {
                total_size += f.size();
                file_count += 1;
            }
        }
    }

    writeln!(
        out,
        "Archive:  {}   {} bytes   {} files",
        args.zipfile.display(),
        total_size,
        file_count
    )?;

    Ok(())
}

/// Print archive trailer with totals
fn print_trailer<R: Read + Seek>(
    out: &mut dyn Write,
    _archive: &mut ZipArchive<R>,
    _args: &Args,
) -> Result<()> {
    // Trailer could show totals, but for now we just add a blank line
    writeln!(out)?;
    Ok(())
}

/// Print file entry in short format (default)
/// Format: -rw-rws---  1.9 unx    2802 t- defX 11-Aug-91 13:48 perms.2660
fn print_short_format(
    out: &mut dyn Write,
    file: &zip::read::ZipFile,
    name: &str,
    datetime_cache: &mut DateTimeCache,
) -> Result<()> {
    let perms = format_permissions(file);
    let version = format_version(file);
    let os = format_os(file);
    let size = file.size();
    let method = format_method(file);
    let datetime = datetime_cache.as_str(file.last_modified());
    let (encrypted, extra) = format_flags(file);
    let mut num_buf = [0u8; 32];
    let size_len = write_u64(&mut num_buf, size);
    let size_str = unsafe { std::str::from_utf8_unchecked(&num_buf[..size_len]) };

    out.write_all(perms.as_bytes())?;
    out.write_all(b"  ")?;
    out.write_all(version.as_bytes())?;
    out.write_all(b" ")?;
    out.write_all(os.as_bytes())?;
    out.write_all(b"  ")?;
    write_right_aligned(out, size_str, 7)?;
    out.write_all(b" ")?;
    out.write_all(&[encrypted as u8, extra as u8])?;
    out.write_all(b" ")?;
    out.write_all(method.as_bytes())?;
    out.write_all(b" ")?;
    out.write_all(datetime.as_bytes())?;
    out.write_all(b" ")?;
    out.write_all(name.as_bytes())?;
    out.write_all(b"\n")?;

    Ok(())
}

/// Print file entry in medium format (with compression percentage)
/// Format: -rw-rws---  1.5 unx    2802 t- 81% defX 11-Aug-91 13:48 perms.2660
fn print_medium_format(
    out: &mut dyn Write,
    file: &zip::read::ZipFile,
    name: &str,
    datetime_cache: &mut DateTimeCache,
) -> Result<()> {
    let perms = format_permissions(file);
    let version = format_version(file);
    let os = format_os(file);
    let size = file.size();
    let ratio = if size > 0 {
        let compressed = file.compressed_size();
        let ratio = (compressed * 100) / size;
        if ratio > 100 {
            0 // Compressed size larger than original (can happen with small files)
        } else {
            100 - ratio
        }
    } else {
        0
    };
    let method = format_method(file);
    let datetime = datetime_cache.as_str(file.last_modified());
    let (encrypted, extra) = format_flags(file);
    let mut num_buf = [0u8; 32];
    let mut num_buf2 = [0u8; 32];
    let size_len = write_u64(&mut num_buf, size);
    let size_str = unsafe { std::str::from_utf8_unchecked(&num_buf[..size_len]) };
    let ratio_len = write_u64(&mut num_buf2, ratio);
    let ratio_str = unsafe { std::str::from_utf8_unchecked(&num_buf2[..ratio_len]) };

    out.write_all(perms.as_bytes())?;
    out.write_all(b"  ")?;
    out.write_all(version.as_bytes())?;
    out.write_all(b" ")?;
    out.write_all(os.as_bytes())?;
    out.write_all(b"  ")?;
    write_right_aligned(out, size_str, 7)?;
    out.write_all(b" ")?;
    out.write_all(&[encrypted as u8, extra as u8])?;
    out.write_all(b" ")?;
    write_right_aligned(out, ratio_str, 2)?;
    out.write_all(b"% ")?;
    out.write_all(method.as_bytes())?;
    out.write_all(b" ")?;
    out.write_all(datetime.as_bytes())?;
    out.write_all(b" ")?;
    out.write_all(name.as_bytes())?;
    out.write_all(b"\n")?;

    Ok(())
}

/// Print file entry in long format (with compressed size)
/// Format: -rw-rws---  1.5 unx    2802 t-     538 defX 11-Aug-91 13:48 perms.2660
fn print_long_format(
    out: &mut dyn Write,
    file: &zip::read::ZipFile,
    name: &str,
    datetime_cache: &mut DateTimeCache,
) -> Result<()> {
    let perms = format_permissions(file);
    let version = format_version(file);
    let os = format_os(file);
    let size = file.size();
    let compressed = file.compressed_size();
    let method = format_method(file);
    let datetime = datetime_cache.as_str(file.last_modified());
    let (encrypted, extra) = format_flags(file);
    let mut num_buf = [0u8; 32];
    let mut num_buf2 = [0u8; 32];
    let size_len = write_u64(&mut num_buf, size);
    let size_str = unsafe { std::str::from_utf8_unchecked(&num_buf[..size_len]) };
    let comp_len = write_u64(&mut num_buf2, compressed);
    let comp_str = unsafe { std::str::from_utf8_unchecked(&num_buf2[..comp_len]) };

    out.write_all(perms.as_bytes())?;
    out.write_all(b"  ")?;
    out.write_all(version.as_bytes())?;
    out.write_all(b" ")?;
    out.write_all(os.as_bytes())?;
    out.write_all(b"  ")?;
    write_right_aligned(out, size_str, 7)?;
    out.write_all(b" ")?;
    out.write_all(&[encrypted as u8, extra as u8])?;
    out.write_all(b" ")?;
    write_right_aligned(out, comp_str, 7)?;
    out.write_all(b" ")?;
    out.write_all(method.as_bytes())?;
    out.write_all(b" ")?;
    out.write_all(datetime.as_bytes())?;
    out.write_all(b" ")?;
    out.write_all(name.as_bytes())?;
    out.write_all(b"\n")?;

    Ok(())
}

/// Print file entry in verbose format (detailed multi-line)
fn print_verbose_format(
    out: &mut dyn Write,
    file: &zip::read::ZipFile,
    name: &str,
    datetime_cache: &mut DateTimeCache,
) -> Result<()> {
    let mut num_buf = [0u8; 32];
    out.write_all(b"File: ")?;
    out.write_all(name.as_bytes())?;
    out.write_all(b"\n")?;

    let comp = file.compressed_size();
    let comp_len = write_u64(&mut num_buf, comp);
    out.write_all(b"  Compressed size:   ")?;
    out.write_all(unsafe { std::str::from_utf8_unchecked(&num_buf[..comp_len]) }.as_bytes())?;
    out.write_all(b"\n")?;

    let size = file.size();
    let size_len = write_u64(&mut num_buf, size);
    out.write_all(b"  Uncompressed size: ")?;
    out.write_all(unsafe { std::str::from_utf8_unchecked(&num_buf[..size_len]) }.as_bytes())?;
    out.write_all(b"\n")?;

    let ratio = if size > 0 {
        let r = (comp * 100) / size;
        if r > 100 { 0 } else { 100 - r }
    } else {
        0
    };
    let ratio_len = write_u64(&mut num_buf, ratio);
    out.write_all(b"  Compression ratio: ")?;
    out.write_all(unsafe { std::str::from_utf8_unchecked(&num_buf[..ratio_len]) }.as_bytes())?;
    out.write_all(b"%\n")?;

    out.write_all(b"  Compression method: ")?;
    out.write_all(format_method(file).as_bytes())?;
    out.write_all(b"\n")?;

    let crc = file.crc32();
    let mut crc_buf = [0u8; 8];
    let mut v = crc;
    for i in (0..8).rev() {
        let digit = (v & 0xF) as u8;
        crc_buf[i] = match digit {
            0..=9 => b'0' + digit,
            _ => b'a' + (digit - 10),
        };
        v >>= 4;
    }
    out.write_all(b"  CRC-32:            ")?;
    out.write_all(&crc_buf)?;
    out.write_all(b"\n")?;

    out.write_all(b"  Modified:          ")?;
    out.write_all(datetime_cache.as_str(file.last_modified()).as_bytes())?;
    out.write_all(b"\n")?;

    out.write_all(b"  OS:                ")?;
    out.write_all(format_os(file).as_bytes())?;
    out.write_all(b"\n")?;

    out.write_all(b"  Version made by:   ")?;
    out.write_all(format_version(file).as_bytes())?;
    out.write_all(b"\n")?;
    if file.encrypted() {
        out.write_all(b"  Encrypted:         Yes\n")?;
    }
    out.write_all(b"\n")?;

    Ok(())
}

/// Format file permissions in Unix style
fn format_permissions(file: &zip::read::ZipFile) -> String {
    #[cfg(unix)]
    {
        if let Some(mode) = file.unix_mode() {
            return format_unix_mode(mode);
        }
    }

    // Default permissions for non-Unix or when not available
    if file.is_dir() {
        "drwxr-xr-x".to_string()
    } else {
        "-rw-r--r--".to_string()
    }
}

#[cfg(unix)]
fn format_unix_mode(mode: u32) -> String {
    let file_type = if mode & 0o040000 != 0 { 'd' } else { '-' };

    let user = format!(
        "{}{}{}",
        if mode & 0o400 != 0 { 'r' } else { '-' },
        if mode & 0o200 != 0 { 'w' } else { '-' },
        if mode & 0o100 != 0 { 'x' } else { '-' }
    );

    let group = format!(
        "{}{}{}",
        if mode & 0o040 != 0 { 'r' } else { '-' },
        if mode & 0o020 != 0 { 'w' } else { '-' },
        if mode & 0o010 != 0 { 'x' } else { '-' }
    );

    let other = format!(
        "{}{}{}",
        if mode & 0o004 != 0 { 'r' } else { '-' },
        if mode & 0o002 != 0 { 'w' } else { '-' },
        if mode & 0o001 != 0 { 'x' } else { '-' }
    );

    format!("{}{}{}{}", file_type, user, group, other)
}

/// Format ZIP version
fn format_version(_file: &zip::read::ZipFile) -> &'static str {
    "2.0" // Most archives use ZIP 2.0 format
}

/// Format host OS
fn format_os(_file: &zip::read::ZipFile) -> &'static str {
    "unx" // Default to Unix
}

/// Format file flags (text/binary, encryption, extra fields)
fn format_flags(file: &zip::read::ZipFile) -> (char, char) {
    let text_binary = 'b'; // Default to binary
    let encrypted = if file.encrypted() {
        text_binary.to_ascii_uppercase()
    } else {
        text_binary
    };
    let extra = '-'; // Would need to check for extended headers/extra fields

    (encrypted, extra)
}

/// Format compression method
fn format_method(file: &zip::read::ZipFile) -> &'static str {
    match file.compression() {
        zip::CompressionMethod::Stored => "stor",
        zip::CompressionMethod::Deflated => "defN", // Default to normal
        zip::CompressionMethod::Bzip2 => "bzp2",
        zip::CompressionMethod::Zstd => "zstd",
        _ => "unkn",
    }
}
