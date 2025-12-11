//! Command-line argument parsing

use clap::Parser;
use std::path::PathBuf;

/// A fast, reliable unzip utility written in Rust - Info-ZIP compatible
#[derive(Parser, Debug, Clone)]
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
  unzip -u archive.zip                 Update (freshen + create new files)")]
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

    /// Quiet mode (-q quieter, -qq quietest)
    #[arg(short = 'q', long = "quiet", action = clap::ArgAction::Count)]
    pub quiet: u8,

    /// Number of parallel extraction threads (default: auto)
    #[arg(short = 'T', long = "threads", value_name = "NUM")]
    pub threads: Option<usize>,

    /// Files to extract (supports glob patterns)
    #[arg(value_name = "PATTERN")]
    pub patterns: Vec<String>,

    /// Exclude files matching these patterns
    #[arg(short = 'x', long = "exclude", value_name = "PATTERN")]
    pub exclude: Vec<String>,
}
