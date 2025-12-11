//! Utility functions for formatting and filtering

use crate::glob::glob_match;
use filetime::FileTime;
use std::time::SystemTime;

/// Format a byte size as human-readable string
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

/// Check if a file should be extracted based on patterns and exclusions
pub fn should_extract(
    name: &str,
    patterns: &[String],
    exclude: &[String],
    case_insensitive: bool,
) -> bool {
    let name_cmp = if case_insensitive {
        name.to_lowercase()
    } else {
        name.to_string()
    };

    // Check exclusions first
    for pattern in exclude {
        let pattern_cmp = if case_insensitive {
            pattern.to_lowercase()
        } else {
            pattern.clone()
        };
        if glob_match(&pattern_cmp, &name_cmp) {
            return false;
        }
    }

    // If no patterns specified, extract all
    if patterns.is_empty() {
        return true;
    }

    // Check if matches any pattern
    for pattern in patterns {
        let pattern_cmp = if case_insensitive {
            pattern.to_lowercase()
        } else {
            pattern.clone()
        };
        if glob_match(&pattern_cmp, &name_cmp) {
            return true;
        }
    }

    false
}

/// Convert ZIP DateTime to SystemTime
pub fn datetime_to_system_time(dt: zip::DateTime) -> SystemTime {
    use std::time::Duration;

    let days_since_epoch = days_from_date(dt.year() as i32, dt.month() as i32, dt.day() as i32);
    let secs = (days_since_epoch as u64) * 86400
        + (dt.hour() as u64) * 3600
        + (dt.minute() as u64) * 60
        + (dt.second() as u64);

    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

/// Convert ZIP DateTime to FileTime
pub fn datetime_to_filetime(dt: zip::DateTime) -> FileTime {
    let days_since_epoch = days_from_date(dt.year() as i32, dt.month() as i32, dt.day() as i32);
    let secs = (days_since_epoch as i64) * 86400
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
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) as u32 + 2) / 5 + day as u32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era as i64) * 146097 + (doe as i64) - 719468
}

/// Format ZIP DateTime as string
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
