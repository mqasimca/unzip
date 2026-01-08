//! Utility functions for formatting and filtering
//!
//! Provides shared helper functions used across the unzip codebase:
//! - Pattern-based file filtering (inclusion and exclusion)
//! - Human-readable size formatting
//! - Timestamp conversion between ZIP and filesystem formats
//!
//! # Pattern Matching
//!
//! File filtering supports both inclusion and exclusion patterns with glob syntax.
//! Multiple patterns can be specified and are evaluated in order.
//!
//! # Examples
//!
//! ```
//! use unzip::{format_size, should_extract};
//!
//! // Size formatting
//! assert_eq!(format_size(1024), "1.0K");
//! assert_eq!(format_size(1536 * 1024), "1.5M");
//!
//! // Pattern matching
//! let includes = vec!["*.txt".to_string()];
//! let excludes = vec![];
//! assert!(should_extract("file.txt", &includes, &excludes, false));
//! ```

use crate::glob::glob_match;
use filetime::FileTime;
use std::time::SystemTime;

/// Format a byte size as a human-readable string with appropriate units.
///
/// Converts raw byte counts to KB, MB, or GB units with one decimal place
/// of precision for better readability in file listings.
///
/// # Arguments
///
/// * `size` - The size in bytes to format
///
/// # Returns
///
/// A formatted string like "1.5M" or "512.0K" or "42B"
///
/// # Examples
///
/// ```
/// use unzip::format_size;
///
/// assert_eq!(format_size(512), "512B");
/// assert_eq!(format_size(1024), "1.0K");
/// assert_eq!(format_size(1536 * 1024), "1.5M");
/// assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.0G");
/// ```
pub fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.1}G", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.1}M", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.1}K", size as f64 / KB as f64)
    } else {
        format!("{}B", size)
    }
}

/// Determine if a file should be extracted based on inclusion/exclusion patterns.
///
/// Evaluates a filename against include and exclude glob patterns to determine
/// if it should be processed. Exclusion patterns are checked first and take
/// precedence over inclusion patterns.
///
/// # Arguments
///
/// * `name` - The filename to check
/// * `patterns` - Include patterns (empty means include all)
/// * `exclude` - Exclude patterns (always applied)
/// * `case_insensitive` - If true, perform case-insensitive matching
///
/// # Returns
///
/// `true` if the file should be extracted, `false` if it should be skipped
///
/// # Logic
///
/// 1. If filename matches any exclusion pattern → return `false`
/// 2. If no inclusion patterns specified → return `true`
/// 3. If filename matches any inclusion pattern → return `true`
/// 4. Otherwise → return `false`
///
/// # Examples
///
/// ```
/// use unzip::should_extract;
///
/// let includes = vec!["*.txt".to_string()];
/// let excludes = vec!["secret*".to_string()];
///
/// assert!(should_extract("file.txt", &includes, &excludes, false));
/// assert!(!should_extract("secret.txt", &includes, &excludes, false));
/// assert!(!should_extract("file.rs", &includes, &excludes, false));
/// ```
pub fn should_extract(
    name: &str,
    patterns: &[String],
    exclude: &[String],
    case_insensitive: bool,
) -> bool {
    let matcher = PatternMatcher::new(patterns, exclude, case_insensitive);
    matcher.should_extract(name)
}

pub(crate) struct PatternMatcher<'a> {
    patterns: &'a [String],
    exclude: &'a [String],
    patterns_ci: Option<Vec<String>>,
    exclude_ci: Option<Vec<String>>,
    case_insensitive: bool,
}

impl<'a> PatternMatcher<'a> {
    pub(crate) fn new(
        patterns: &'a [String],
        exclude: &'a [String],
        case_insensitive: bool,
    ) -> Self {
        let patterns_ci = if case_insensitive {
            Some(patterns.iter().map(|p| p.to_lowercase()).collect())
        } else {
            None
        };
        let exclude_ci = if case_insensitive {
            Some(exclude.iter().map(|p| p.to_lowercase()).collect())
        } else {
            None
        };
        Self {
            patterns,
            exclude,
            patterns_ci,
            exclude_ci,
            case_insensitive,
        }
    }

    pub(crate) fn should_extract(&self, name: &str) -> bool {
        if self.patterns.is_empty() && self.exclude.is_empty() {
            return true;
        }

        if self.case_insensitive {
            let name_cmp = name.to_lowercase();
            let exclude = self.exclude_ci.as_deref().unwrap_or(&[]);
            for pattern in exclude {
                if glob_match(pattern, &name_cmp) {
                    return false;
                }
            }

            let patterns = self.patterns_ci.as_deref().unwrap_or(&[]);
            if patterns.is_empty() {
                return true;
            }

            for pattern in patterns {
                if glob_match(pattern, &name_cmp) {
                    return true;
                }
            }

            false
        } else {
            for pattern in self.exclude {
                if glob_match(pattern, name) {
                    return false;
                }
            }

            if self.patterns.is_empty() {
                return true;
            }

            for pattern in self.patterns {
                if glob_match(pattern, name) {
                    return true;
                }
            }

            false
        }
    }
}

/// Convert ZIP DateTime format to Rust SystemTime.
///
/// Converts the date/time format used in ZIP archives (year, month, day, hour,
/// minute, second) to Rust's standard SystemTime for setting file modification times.
///
/// # Arguments
///
/// * `dt` - The ZIP DateTime to convert
///
/// # Returns
///
/// A SystemTime representing the same instant
///
/// # Examples
///
/// ```
/// use zip::DateTime;
/// use unzip::utils::datetime_to_system_time;
///
/// let dt = DateTime::from_date_and_time(2024, 1, 15, 10, 30, 0).unwrap();
/// let sys_time = datetime_to_system_time(dt);
/// // sys_time now represents 2024-01-15 10:30:00
/// ```
pub fn datetime_to_system_time(dt: zip::DateTime) -> SystemTime {
    use std::time::Duration;

    let days_since_epoch = days_from_date(dt.year() as i32, dt.month() as i32, dt.day() as i32);
    let secs = (days_since_epoch as u64) * 86400
        + (dt.hour() as u64) * 3600
        + (dt.minute() as u64) * 60
        + (dt.second() as u64);

    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

/// Convert ZIP DateTime format to filetime::FileTime.
///
/// Converts ZIP archive timestamps to the FileTime type used for setting
/// file modification times on disk via the filetime crate.
///
/// # Arguments
///
/// * `dt` - The ZIP DateTime to convert
///
/// # Returns
///
/// A FileTime representing the same instant, suitable for use with
/// `filetime::set_file_mtime()`
///
/// # Examples
///
/// ```no_run
/// use zip::DateTime;
/// use unzip::utils::datetime_to_filetime;
/// use std::path::Path;
///
/// let dt = DateTime::from_date_and_time(2024, 1, 15, 10, 30, 0).unwrap();
/// let ft = datetime_to_filetime(dt);
/// // Can now use: filetime::set_file_mtime(path, ft)?;
/// ```
pub fn datetime_to_filetime(dt: zip::DateTime) -> FileTime {
    let days_since_epoch = days_from_date(dt.year() as i32, dt.month() as i32, dt.day() as i32);
    let secs = days_since_epoch * 86400
        + (dt.hour() as i64) * 3600
        + (dt.minute() as i64) * 60
        + (dt.second() as i64);

    FileTime::from_unix_time(secs, 0)
}

/// Calculate days from date using Howard Hinnant's algorithm
fn days_from_date(year: i32, month: i32, day: i32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy =
        (153 * (if month > 2 { month - 3 } else { month + 9 }) as u32 + 2) / 5 + day as u32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era as i64) * 146097 + (doe as i64) - 719468
}

/// Format a ZIP DateTime as a human-readable string.
///
/// Converts an optional ZIP DateTime to a formatted string suitable for
/// display in file listings. Returns "N/A" if no datetime is available.
///
/// # Arguments
///
/// * `datetime` - Optional ZIP DateTime to format
///
/// # Returns
///
/// A formatted string like "2024-01-15 10:30:00", or fixed-width spaces if None
/// (for alignment in file listings)
///
/// # Examples
///
/// ```
/// use zip::DateTime;
/// use unzip::utils::format_datetime;
///
/// let dt = DateTime::from_date_and_time(2024, 1, 15, 10, 30, 0).unwrap();
/// assert_eq!(format_datetime(Some(dt)), "2024-01-15 10:30:00");
/// // None returns fixed-width space padding for alignment in listings
/// assert_eq!(format_datetime(None), "                   ");
/// ```
pub fn format_datetime(datetime: Option<zip::DateTime>) -> String {
    match datetime {
        Some(dt) => format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            dt.year(),
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second()
        ),
        None => "                   ".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(1), "1B");
        assert_eq!(format_size(500), "500B");
        assert_eq!(format_size(1023), "1023B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0K");
        assert_eq!(format_size(1536), "1.5K");
        assert_eq!(format_size(2048), "2.0K");
        assert_eq!(format_size(1024 * 1023), "1023.0K");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.0M");
        assert_eq!(format_size(1024 * 1024 * 2), "2.0M");
        assert_eq!(format_size(1024 * 1024 + 512 * 1024), "1.5M");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0G");
        assert_eq!(format_size(1024 * 1024 * 1024 * 2), "2.0G");
    }

    #[test]
    fn test_should_extract_no_patterns() {
        assert!(should_extract("file.txt", &[], &[], false));
        assert!(should_extract("any/path/file.rs", &[], &[], false));
    }

    #[test]
    fn test_should_extract_with_patterns() {
        let patterns = vec!["*.txt".to_string()];
        assert!(should_extract("file.txt", &patterns, &[], false));
        assert!(!should_extract("file.rs", &patterns, &[], false));
    }

    #[test]
    fn test_should_extract_with_exclude() {
        let exclude = vec!["*.log".to_string()];
        assert!(should_extract("file.txt", &[], &exclude, false));
        assert!(!should_extract("debug.log", &[], &exclude, false));
    }

    #[test]
    fn test_should_extract_case_insensitive() {
        let patterns = vec!["*.TXT".to_string()];
        assert!(!should_extract("file.txt", &patterns, &[], false));
        assert!(should_extract("file.txt", &patterns, &[], true));
        assert!(should_extract("FILE.TXT", &patterns, &[], true));
    }

    #[test]
    fn test_should_extract_exclude_takes_priority() {
        let patterns = vec!["*.txt".to_string()];
        let exclude = vec!["secret.txt".to_string()];
        assert!(should_extract("file.txt", &patterns, &exclude, false));
        assert!(!should_extract("secret.txt", &patterns, &exclude, false));
    }
}
