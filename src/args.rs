//! Command-line argument parsing
//!
//! Defines the CLI interface for the unzip utility using clap. Provides Info-ZIP
//! compatible options for extraction, listing, testing, and filtering operations.
//!
//! # Compatibility
//!
//! Aims for command-line compatibility with Info-ZIP unzip, supporting common
//! options like `-l`, `-v`, `-t`, `-d`, `-o`, `-n`, `-f`, `-u`, `-j`, and `-x`.
//!
//! # Examples
//!
//! ```no_run
//! use unzip::Args;
//! use clap::Parser;
//!
//! // Parse from command line
//! let args = Args::parse();
//! ```

use clap::Parser;
use std::path::PathBuf;

/// A fast, reliable unzip utility written in Rust - Info-ZIP compatible
#[derive(Parser, Debug, Clone, Default)]
#[command(author, version, about, long_about = None)]
#[command(after_help = "Examples:
  unzip archive.zip                    Extract all files to current directory
  unzip -l archive.zip                 List contents without extracting
  unzip -t archive.zip                 Test archive integrity
  unzip -d /tmp archive.zip            Extract to /tmp directory
  unzip archive.zip '*.txt'            Extract only .txt files
  unzip archive.zip -x '*.log'         Extract all except .log files
  unzip -p archive.zip file.txt        Extract file.txt to stdout
  unzip -j archive.zip                 Extract without directory structure
  unzip -o archive.zip                 Overwrite files without prompting
  unzip -n archive.zip                 Never overwrite existing files
  unzip -f archive.zip                 Freshen (update only existing files)
  unzip -u archive.zip                 Update (freshen + create new files)
  unzip -Z archive.zip                 Zipinfo mode: detailed archive information
  unzip -Z -v archive.zip              Verbose zipinfo output")]
pub struct Args {
    /// Path to the ZIP file to extract
    #[arg(value_name = "FILE")]
    pub zipfile: PathBuf,

    /// Directory to extract files to (default: current directory)
    #[arg(short = 'd', long = "directory", value_name = "EXDIR")]
    pub output_dir: Option<PathBuf>,

    /// List contents only (short format)
    #[arg(short = 'l', long = "list")]
    pub list_only: bool,

    /// List contents verbosely
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Test archive integrity
    #[arg(short = 't', long = "test")]
    pub test: bool,

    /// Extract files to stdout/pipe (no messages)
    #[arg(short = 'p', long = "pipe")]
    pub pipe: bool,

    /// Display archive comment only
    #[arg(short = 'z', long = "comment")]
    pub comment_only: bool,

    /// Zipinfo mode: detailed archive information (-Z or -Z MODE)
    /// Modes: -1 (filenames), -2 (filenames+headers), -s (short, default),
    /// -m (medium with %), -l (long with size), -v (verbose), -h (header), -t (trailer)
    #[arg(short = 'Z', long = "zipinfo", value_name = "MODE")]
    pub zipinfo: Option<Option<String>>,

    /// Overwrite existing files without prompting
    #[arg(short = 'o', long = "overwrite")]
    pub overwrite: bool,

    /// Never overwrite existing files
    #[arg(short = 'n', long = "never-overwrite")]
    pub never_overwrite: bool,

    /// Freshen existing files only (don't create new)
    #[arg(short = 'f', long = "freshen")]
    pub freshen: bool,

    /// Update files (freshen + create if necessary)
    #[arg(short = 'u', long = "update")]
    pub update: bool,

    /// Junk paths (don't create directories)
    #[arg(short = 'j', long = "junk-paths")]
    pub junk_paths: bool,

    /// Match filenames case-insensitively
    #[arg(short = 'C', long = "case-insensitive")]
    pub case_insensitive: bool,

    /// Make filenames lowercase
    #[arg(short = 'L', long = "lowercase")]
    pub lowercase: bool,

    /// Skip restoring file timestamps
    #[arg(short = 'D', long = "no-timestamps")]
    pub no_timestamps: bool,

    /// Quiet mode (-q quieter, -qq quietest)
    #[arg(short = 'q', long = "quiet", action = clap::ArgAction::Count)]
    pub quiet: u8,

    /// Number of parallel extraction threads (default: auto)
    #[arg(short = 'T', long = "threads", value_name = "NUM")]
    pub threads: Option<usize>,

    /// Password for encrypted files (insecure, use interactive prompt instead)
    #[arg(short = 'P', long = "password", value_name = "PASSWORD")]
    pub password: Option<String>,

    /// Files to extract (supports glob patterns)
    #[arg(value_name = "PATTERN")]
    pub patterns: Vec<String>,

    /// Exclude files matching these patterns
    #[arg(short = 'x', long = "exclude", value_name = "PATTERN")]
    pub exclude: Vec<String>,
}
