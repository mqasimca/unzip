//! Glob pattern matching for file selection
//!
//! Provides Info-ZIP compatible glob pattern matching for filtering files during
//! extraction. Supports standard wildcard operators and recursive matching.
//!
//! # Supported Patterns
//!
//! - `*` - Matches zero or more characters, but not directory separator `/`
//! - `**` - Matches zero or more characters, including directory separator `/`
//! - `?` - Matches exactly one character, but not directory separator `/`
//!
//! # Algorithm
//!
//! Uses backtracking algorithm to handle complex wildcard patterns efficiently.
//! The implementation is optimized for common cases (no wildcards, single wildcards)
//! while correctly handling nested and adjacent wildcards.
//!
//! # Examples
//!
//! ```
//! use unzip::glob_match;
//!
//! assert!(glob_match("*.txt", "file.txt"));
//! assert!(glob_match("src/**/*.rs", "src/main.rs"));
//! assert!(glob_match("test?.dat", "test1.dat"));
//! assert!(!glob_match("*.txt", "file.rs"));
//! ```

/// Match a glob pattern against text
///
/// Supports:
/// - `*` matches any characters except `/`
/// - `**` matches any characters including `/`
/// - `?` matches any single character except `/`
pub fn glob_match(pattern: &str, text: &str) -> bool {
    glob_match_impl(pattern.as_bytes(), text.as_bytes())
}

fn glob_match_impl(pattern: &[u8], text: &[u8]) -> bool {
    let mut px = 0;
    let mut tx = 0;
    let mut next_px = 0;
    let mut next_tx = 0;

    while tx < text.len() || px < pattern.len() {
        if px < pattern.len() {
            match pattern[px] {
                b'*' => {
                    // Check for **
                    if px + 1 < pattern.len() && pattern[px + 1] == b'*' {
                        // ** matches everything including /
                        px += 2;
                        // Skip optional /
                        if px < pattern.len() && pattern[px] == b'/' {
                            px += 1;
                        }
                        if px >= pattern.len() {
                            return true;
                        }
                        // Try to match rest at every position
                        for i in tx..=text.len() {
                            if glob_match_impl(&pattern[px..], &text[i..]) {
                                return true;
                            }
                        }
                        return false;
                    }
                    // Single * - save state for backtracking
                    next_px = px;
                    next_tx = tx + 1;
                    px += 1;
                    continue;
                },
                b'?' => {
                    if tx < text.len() && text[tx] != b'/' {
                        px += 1;
                        tx += 1;
                        continue;
                    }
                },
                c => {
                    if tx < text.len() && text[tx] == c {
                        px += 1;
                        tx += 1;
                        continue;
                    }
                },
            }
        }

        // Mismatch - try backtracking
        if next_tx > 0 && next_tx <= text.len() {
            // Don't let * match /
            if text[next_tx - 1] == b'/' {
                return false;
            }
            px = next_px;
            tx = next_tx;
            next_tx += 1;
            continue;
        }

        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_simple_wildcard() {
        assert!(glob_match("*.txt", "file.txt"));
        assert!(glob_match("*.txt", "document.txt"));
        assert!(!glob_match("*.txt", "file.rs"));
        assert!(!glob_match("*.txt", "file.txt.bak"));
    }

    #[test]
    fn test_glob_match_prefix_wildcard() {
        assert!(glob_match("file.*", "file.txt"));
        assert!(glob_match("file.*", "file.rs"));
        assert!(!glob_match("file.*", "myfile.txt"));
    }

    #[test]
    fn test_glob_match_middle_wildcard() {
        assert!(glob_match("file*.txt", "file.txt"));
        assert!(glob_match("file*.txt", "file123.txt"));
        assert!(glob_match("file*.txt", "file_name.txt"));
        assert!(!glob_match("file*.txt", "myfile.txt"));
    }

    #[test]
    fn test_glob_match_question_mark() {
        assert!(glob_match("file?.txt", "file1.txt"));
        assert!(glob_match("file?.txt", "filea.txt"));
        assert!(!glob_match("file?.txt", "file12.txt"));
        assert!(!glob_match("file?.txt", "file.txt"));
    }

    #[test]
    fn test_glob_match_double_star() {
        assert!(glob_match("**/*.rs", "src/main.rs"));
        assert!(glob_match("**/*.rs", "src/lib/mod.rs"));
        assert!(glob_match("**/*.rs", "main.rs"));
        assert!(!glob_match("**/*.rs", "src/main.txt"));
    }

    #[test]
    fn test_glob_match_single_star_no_slash() {
        assert!(glob_match("src/*", "src/main.rs"));
        assert!(!glob_match("src/*", "src/lib/mod.rs"));
    }

    #[test]
    fn test_glob_match_directory_pattern() {
        assert!(glob_match("src/**", "src/main.rs"));
        assert!(glob_match("src/**", "src/lib/mod.rs"));
        assert!(glob_match("src/**", "src/a/b/c/d.rs"));
        assert!(!glob_match("src/**", "test/main.rs"));
    }

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("file.txt", "file.txt"));
        assert!(!glob_match("file.txt", "file.rs"));
        assert!(!glob_match("file.txt", "other.txt"));
    }

    #[test]
    fn test_glob_match_empty_pattern() {
        assert!(glob_match("", ""));
        assert!(!glob_match("", "file.txt"));
    }

    #[test]
    fn test_glob_match_only_wildcard() {
        assert!(glob_match("*", "file.txt"));
        assert!(glob_match("*", "anything"));
        assert!(!glob_match("*", "path/file.txt"));
    }

    #[test]
    fn test_glob_match_double_star_only() {
        assert!(glob_match("**", "file.txt"));
        assert!(glob_match("**", "path/file.txt"));
        assert!(glob_match("**", "a/b/c/d/e.txt"));
    }
}
