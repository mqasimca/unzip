//! A fast, reliable unzip utility written in Rust - Info-ZIP compatible
//!
//! # Overview
//!
//! This is a high-performance unzip implementation that aims for ~5x speed improvement
//! over Info-ZIP unzip while maintaining CLI compatibility. Performance gains come from:
//!
//! - Memory-mapped I/O for files >1MB
//! - Linux kernel optimizations (madvise, fallocate, fadvise)
//! - Efficient buffering (256KB buffers)
//! - Minimal allocations
//!
//! # Architecture
//!
//! The main entry point handles:
//! 1. CLI argument parsing and validation
//! 2. File opening and memory mapping decisions
//! 3. Dispatching to appropriate operation (list, test, extract, pipe)
//!
//! Files >1MB use memory mapping for better performance, while smaller files
//! use traditional file I/O to avoid mmap overhead.

use anyhow::{Context, Result, bail};
use clap::Parser;
use memmap2::Mmap;
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use zip::ZipArchive;

use unzip::args::Args;
use unzip::extract::{extract_archive, extract_to_pipe};
use unzip::linux::{fadvise_sequential, madvise_sequential};
use unzip::list::{display_comment, list_contents};
use unzip::test_archive::test_archive;
use unzip::zipinfo::display_zipinfo;

fn main() -> Result<()> {
    let args = Args::parse();

    if args.overwrite && args.never_overwrite {
        bail!("Cannot specify both -o (overwrite) and -n (never overwrite)");
    }

    let file = File::open(&args.zipfile)
        .with_context(|| format!("Failed to open ZIP file: {}", args.zipfile.display()))?;

    let file_size = file.metadata()?.len();

    if file_size > 1024 * 1024 {
        // Linux optimization: hint kernel about sequential access
        fadvise_sequential(&file, file_size);

        let mmap = unsafe { Mmap::map(&file) }.with_context(|| "Failed to memory-map file")?;

        // Linux optimization: tell kernel we'll read sequentially
        madvise_sequential(mmap.as_ptr(), mmap.len());

        let cursor = Cursor::new(&mmap[..]);
        let mut archive = ZipArchive::new(cursor)
            .with_context(|| format!("Failed to read ZIP archive: {}", args.zipfile.display()))?;
        run_command(&mut archive, &args)
    } else {
        // For smaller files, still hint sequential access
        fadvise_sequential(&file, file_size);

        let mut archive = ZipArchive::new(file)
            .with_context(|| format!("Failed to read ZIP archive: {}", args.zipfile.display()))?;
        run_command(&mut archive, &args)
    }
}

fn run_command<R: Read + Seek>(archive: &mut ZipArchive<R>, args: &Args) -> Result<()> {
    if args.zipinfo.is_some() {
        display_zipinfo(archive, args)?;
    } else if args.comment_only {
        display_comment(archive)?;
    } else if args.list_only || args.verbose {
        list_contents(archive, args.verbose)?;
    } else if args.test {
        test_archive(archive, args)?;
    } else if args.pipe {
        extract_to_pipe(archive, args)?;
    } else {
        extract_archive(archive, args)?;
    }
    Ok(())
}
