//! A fast, reliable unzip utility written in Rust - Info-ZIP compatible

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
