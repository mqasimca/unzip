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
use std::io::{Read, Seek};
use zip::ZipArchive;

use crate::args::Args;
use crate::utils::{format_datetime, should_extract};

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
    // Determine mode from zipinfo argument
    let mode = if let Some(Some(mode_str)) = &args.zipinfo {
        ZipinfoMode::from_str(mode_str).unwrap_or(ZipinfoMode::Short)
    } else {
        ZipinfoMode::Short // Default mode
    };

    // Print header (except for FilenamesOnly mode)
    if mode != ZipinfoMode::FilenamesOnly && args.quiet == 0 {
        print_header(archive, args)?;
    }

    // Print file entries
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let name = file.name().to_string();

        if !should_extract(&name, &args.patterns, &args.exclude, args.case_insensitive) {
            continue;
        }

        match mode {
            ZipinfoMode::FilenamesOnly | ZipinfoMode::FilenamesWithHeaders => {
                println!("{}", name);
            },
            ZipinfoMode::Short => {
                print_short_format(&file, &name)?;
            },
            ZipinfoMode::Medium => {
                print_medium_format(&file, &name)?;
            },
            ZipinfoMode::Long => {
                print_long_format(&file, &name)?;
            },
            ZipinfoMode::Verbose => {
                print_verbose_format(&file, &name)?;
            },
        }
    }

    // Print trailer (except for FilenamesOnly mode)
    if mode != ZipinfoMode::FilenamesOnly && args.quiet == 0 {
        print_trailer(archive, args)?;
    }

    Ok(())
}

/// Print archive header with summary information
fn print_header<R: Read + Seek>(archive: &mut ZipArchive<R>, args: &Args) -> Result<()> {
    let total_size: u64 = (0..archive.len())
        .filter_map(|i| {
            archive.by_index(i).ok().and_then(|f| {
                let name = f.name().to_string();
                if should_extract(&name, &args.patterns, &args.exclude, args.case_insensitive) {
                    Some(f.size())
                } else {
                    None
                }
            })
        })
        .sum();

    let file_count: usize = (0..archive.len())
        .filter(|&i| {
            archive
                .by_index(i)
                .ok()
                .map(|f| {
                    let name = f.name().to_string();
                    should_extract(&name, &args.patterns, &args.exclude, args.case_insensitive)
                })
                .unwrap_or(false)
        })
        .count();

    println!(
        "Archive:  {}   {} bytes   {} files",
        args.zipfile.display(),
        total_size,
        file_count
    );

    Ok(())
}

/// Print archive trailer with totals
fn print_trailer<R: Read + Seek>(_archive: &mut ZipArchive<R>, _args: &Args) -> Result<()> {
    // Trailer could show totals, but for now we just add a blank line
    println!();
    Ok(())
}

/// Print file entry in short format (default)
/// Format: -rw-rws---  1.9 unx    2802 t- defX 11-Aug-91 13:48 perms.2660
fn print_short_format(file: &zip::read::ZipFile, name: &str) -> Result<()> {
    let perms = format_permissions(file);
    let version = format_version(file);
    let os = format_os(file);
    let size = file.size();
    let flags = format_flags(file);
    let method = format_method(file);
    let datetime = format_datetime(file.last_modified());

    println!(
        "{}  {} {}  {:>7} {} {} {} {}",
        perms, version, os, size, flags, method, datetime, name
    );

    Ok(())
}

/// Print file entry in medium format (with compression percentage)
/// Format: -rw-rws---  1.5 unx    2802 t- 81% defX 11-Aug-91 13:48 perms.2660
fn print_medium_format(file: &zip::read::ZipFile, name: &str) -> Result<()> {
    let perms = format_permissions(file);
    let version = format_version(file);
    let os = format_os(file);
    let size = file.size();
    let flags = format_flags(file);
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
    let datetime = format_datetime(file.last_modified());

    println!(
        "{}  {} {}  {:>7} {} {:>2}% {} {} {}",
        perms, version, os, size, flags, ratio, method, datetime, name
    );

    Ok(())
}

/// Print file entry in long format (with compressed size)
/// Format: -rw-rws---  1.5 unx    2802 t-     538 defX 11-Aug-91 13:48 perms.2660
fn print_long_format(file: &zip::read::ZipFile, name: &str) -> Result<()> {
    let perms = format_permissions(file);
    let version = format_version(file);
    let os = format_os(file);
    let size = file.size();
    let flags = format_flags(file);
    let compressed = file.compressed_size();
    let method = format_method(file);
    let datetime = format_datetime(file.last_modified());

    println!(
        "{}  {} {}  {:>7} {} {:>7} {} {} {}",
        perms, version, os, size, flags, compressed, method, datetime, name
    );

    Ok(())
}

/// Print file entry in verbose format (detailed multi-line)
fn print_verbose_format(file: &zip::read::ZipFile, name: &str) -> Result<()> {
    println!("File: {}", name);
    println!("  Compressed size:   {}", file.compressed_size());
    println!("  Uncompressed size: {}", file.size());
    println!("  Compression ratio: {}%", {
        let size = file.size();
        if size > 0 {
            let ratio = (file.compressed_size() * 100) / size;
            if ratio > 100 { 0 } else { 100 - ratio }
        } else {
            0
        }
    });
    println!("  Compression method: {}", format_method(file));
    println!("  CRC-32:            {:08x}", file.crc32());
    println!("  Modified:          {}", format_datetime(file.last_modified()));
    println!("  OS:                {}", format_os(file));
    println!("  Version made by:   {}", format_version(file));
    if file.encrypted() {
        println!("  Encrypted:         Yes");
    }
    println!();

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
fn format_version(_file: &zip::read::ZipFile) -> String {
    "2.0".to_string() // Most archives use ZIP 2.0 format
}

/// Format host OS
fn format_os(_file: &zip::read::ZipFile) -> String {
    "unx".to_string() // Default to Unix
}

/// Format file flags (text/binary, encryption, extra fields)
fn format_flags(file: &zip::read::ZipFile) -> String {
    let text_binary = 'b'; // Default to binary
    let encrypted = if file.encrypted() {
        text_binary.to_ascii_uppercase()
    } else {
        text_binary
    };
    let extra = '-'; // Would need to check for extended headers/extra fields

    format!("{}{}", encrypted, extra)
}

/// Format compression method
fn format_method(file: &zip::read::ZipFile) -> String {
    match file.compression() {
        zip::CompressionMethod::Stored => "stor".to_string(),
        zip::CompressionMethod::Deflated => "defN".to_string(), // Default to normal
        zip::CompressionMethod::Bzip2 => "bzp2".to_string(),
        zip::CompressionMethod::Zstd => "zstd".to_string(),
        _ => "unkn".to_string(),
    }
}
