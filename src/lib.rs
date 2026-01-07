//! A fast, reliable unzip utility written in Rust - Info-ZIP compatible
//!
//! This library provides the core functionality for a high-performance ZIP extraction
//! utility. It can be used as both a command-line tool and as a library for embedding
//! ZIP extraction capabilities in other applications.
//!
//! # Features
//!
//! - Info-ZIP compatible command-line interface
//! - Multiple overwrite modes (always, never, freshen, update)
//! - Pattern-based file filtering with glob support
//! - Archive listing and integrity testing
//! - Progress reporting
//! - File timestamp and permission preservation
//! - Linux kernel optimizations for maximum throughput
//!
//! # Performance
//!
//! Optimized for ~5x speed improvement over Info-ZIP unzip through:
//! - Memory-mapped I/O for large files (>1MB)
//! - Linux optimizations (madvise, fallocate, fadvise)
//! - 256KB I/O buffers
//! - Minimal memory allocations
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

pub mod args;
pub mod extract;
pub mod glob;
pub mod linux;
pub mod list;
pub mod test_archive;
pub mod utils;

pub use args::Args;
pub use extract::extract_archive;
pub use glob::glob_match;
pub use list::{display_comment, list_contents};
pub use test_archive::test_archive;
pub use utils::{format_size, should_extract};
