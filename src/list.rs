//! Archive listing functionality

use anyhow::Result;
use std::io::{Read, Seek};
use zip::ZipArchive;

use crate::utils::{format_datetime, format_size};

/// Display archive comment
pub fn display_comment<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<()> {
    let comment = archive.comment();
    if !comment.is_empty() {
        println!("{}", String::from_utf8_lossy(comment));
    }
    Ok(())
}

/// List archive contents
pub fn list_contents<R: Read + Seek>(archive: &mut ZipArchive<R>, verbose: bool) -> Result<()> {
    if verbose {
        println!(
            "{:>8}  {:>8}  {:>5}  {:>19}  {:>8}  {}",
            "Length", "Size", "Ratio", "Date & Time", "CRC-32", "Name"
        );
        println!("{}", "-".repeat(80));
    } else {
        println!("{:>10}  {:>19}  {}", "Size", "Modified", "Name");
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
        println!(
            "{:>10}  {:>19}  {} files",
            format_size(total_size),
            "",
            file_count
        );
    }

    Ok(())
}
