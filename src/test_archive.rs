//! Archive integrity testing

use anyhow::{bail, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{Read, Seek};
use std::sync::atomic::{AtomicUsize, Ordering};
use zip::ZipArchive;

use crate::args::Args;
use crate::utils::should_extract;

/// Test archive integrity by verifying CRC checksums
pub fn test_archive<R: Read + Seek>(archive: &mut ZipArchive<R>, args: &Args) -> Result<()> {
    let total_files = archive.len();
    let errors = AtomicUsize::new(0);
    let tested = AtomicUsize::new(0);

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

        // Check if file matches patterns
        if !should_extract(&name, &args.patterns, &args.exclude, args.case_insensitive) {
            if let Some(ref pb) = progress_bar {
                pb.inc(1);
            }
            continue;
        }

        // Read and verify CRC
        let mut buffer = Vec::new();
        if let Err(e) = file.read_to_end(&mut buffer) {
            if args.quiet < 2 {
                eprintln!("error: {} - {}", name, e);
            }
            errors.fetch_add(1, Ordering::Relaxed);
        } else {
            let computed_crc = crc32fast::hash(&buffer);
            let stored_crc = file.crc32();

            if computed_crc != stored_crc {
                if args.quiet < 2 {
                    eprintln!(
                        "error: {} - CRC mismatch (stored: {:08x}, computed: {:08x})",
                        name, stored_crc, computed_crc
                    );
                }
                errors.fetch_add(1, Ordering::Relaxed);
            } else if args.quiet == 0 {
                if let Some(ref pb) = progress_bar {
                    pb.println(format!("    testing: {}  OK", name));
                }
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
