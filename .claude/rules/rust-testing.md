---
paths: src/**/*.rs, tests/**/*.rs
---

# Rust Testing Rules

These rules apply to all test code in the unzip project.

## Test Coverage Requirements

### Minimum Coverage by Module

| Module | Target Coverage | Current | Priority |
|--------|-----------------|---------|----------|
| extract.rs | 80% | ~80% | ✓ Maintained |
| glob.rs | 90% | ~90% | ✓ Maintained |
| utils.rs | 80% | ~80% | ✓ Maintained |
| list.rs | 70% | 0% | ⚠️ CRITICAL |
| test_archive.rs | 70% | 0% | ⚠️ CRITICAL |
| linux.rs | N/A | N/A | Platform-specific |
| args.rs | N/A | N/A | Struct definition |
| main.rs | N/A | N/A | Integration layer |

### Overall Target
**Minimum 60% code coverage** across all testable modules.

## Test Organization

### Unit Tests (in same file)

Place unit tests in a `#[cfg(test)]` module at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Test implementation
    }
}
```

**Location:** Same file as code under test

**Purpose:** Test individual functions and methods

### Integration Tests (separate files)

Place in `tests/` directory:

```rust
// tests/integration_test.rs
use unzip::extract_archive;

#[test]
fn test_end_to_end_extraction() {
    // Test full workflow
}
```

**Location:** `tests/` directory

**Purpose:** Test complete workflows and public APIs

### Doc Tests (in documentation)

Include in doc comments:

```rust
/// Extract files from an archive.
///
/// # Examples
///
/// ```
/// # use unzip::extract_archive;
/// let result = extract_archive("test.zip");
/// assert!(result.is_ok());
/// ```
pub fn extract_archive(path: &str) -> Result<()> {
    // ...
}
```

**Location:** Documentation comments

**Purpose:** Show usage examples that are guaranteed to compile

## Test Naming Convention

Use this pattern:
```
test_<function>_<condition>_<expected_result>
```

Examples:
```rust
#[test]
fn test_glob_match_wildcard_succeeds() { }

#[test]
fn test_glob_match_no_match_fails() { }

#[test]
fn test_extract_nonexistent_file_returns_error() { }

#[test]
fn test_extract_with_overwrite_replaces_file() { }

#[test]
fn test_format_size_zero_returns_zero_bytes() { }
```

### Good Test Names
- Describe what is being tested
- State the condition/input
- Indicate expected outcome
- Are self-documenting

### Bad Test Names
```rust
#[test]
fn test1() { }  // Too vague

#[test]
fn test_extract() { }  // What about extract?

#[test]
fn it_works() { }  // What works?

#[test]
fn test_bug_fix() { }  // Which bug?
```

## Test Structure

### Arrange-Act-Assert Pattern

```rust
#[test]
fn test_extract_with_pattern() {
    // Arrange: Set up test data
    let archive = create_test_archive();
    let pattern = "*.txt";

    // Act: Perform operation
    let result = extract_with_pattern(&archive, pattern);

    // Assert: Verify results
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 3);
}
```

### One Assertion Per Test (Guideline)

Prefer focused tests:

```rust
// Good: Focused test
#[test]
fn test_glob_match_wildcard_matches_txt() {
    assert!(glob_match("*.txt", "file.txt"));
}

#[test]
fn test_glob_match_wildcard_rejects_rs() {
    assert!(!glob_match("*.txt", "file.rs"));
}

// Acceptable: Related assertions
#[test]
fn test_glob_match_wildcard() {
    assert!(glob_match("*.txt", "file.txt"));
    assert!(glob_match("*.txt", "doc.txt"));
    assert!(!glob_match("*.txt", "file.rs"));
}
```

## What to Test

### Test Both Success and Error Paths

```rust
#[test]
fn test_extract_valid_archive_succeeds() {
    let result = extract("valid.zip");
    assert!(result.is_ok());
}

#[test]
fn test_extract_invalid_archive_returns_error() {
    let result = extract("invalid.zip");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to read"));
}
```

### Test Edge Cases

```rust
#[test]
fn test_glob_match_empty_pattern() {
    assert!(glob_match("", ""));
    assert!(!glob_match("", "text"));
}

#[test]
fn test_glob_match_empty_text() {
    assert!(!glob_match("*", ""));
}

#[test]
fn test_format_size_zero() {
    assert_eq!(format_size(0), "0 bytes");
}

#[test]
fn test_format_size_max_value() {
    assert_eq!(format_size(u64::MAX), "16 EB");
}
```

### Test Boundary Conditions

```rust
#[test]
fn test_buffer_size_exactly_1mb() {
    // Test mmap threshold (main.rs:30)
    let size = 1024 * 1024;
    let should_use_mmap = size > 1024 * 1024;
    assert!(!should_use_mmap);  // Exactly 1MB should NOT use mmap
}

#[test]
fn test_buffer_size_just_over_1mb() {
    let size = 1024 * 1024 + 1;
    let should_use_mmap = size > 1024 * 1024;
    assert!(should_use_mmap);  // Just over should use mmap
}
```

## Test Assertions

### Use Appropriate Assertion Macros

```rust
// Equality
assert_eq!(actual, expected);
assert_eq!(actual, expected, "Custom message: {}", value);

// Inequality
assert_ne!(actual, unexpected);

// Boolean conditions
assert!(condition);
assert!(condition, "Expected condition to be true");

// Negation
assert!(!condition);
```

### Testing Results

```rust
// Test for Ok
assert!(result.is_ok());

// Test for specific Ok value
assert_eq!(result.unwrap(), expected_value);

// Test for Err
assert!(result.is_err());

// Test error message
let err = result.unwrap_err();
assert!(err.to_string().contains("expected text"));
```

### Testing Options

```rust
// Test for Some
assert!(option.is_some());

// Test for None
assert!(option.is_none());

// Test Some value
assert_eq!(option.unwrap(), expected);
```

## Test Fixtures and Helpers

### Use Temporary Files/Directories

```rust
use tempfile::{NamedTempFile, TempDir};

#[test]
fn test_with_temp_file() {
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();

    // Use temp file
    write_data(path);
    let data = read_data(path);

    assert_eq!(data, expected);
    // Temp file automatically cleaned up
}

#[test]
fn test_with_temp_directory() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    // Use temp directory
    create_file(&file_path);

    assert!(file_path.exists());
    // Temp directory automatically cleaned up
}
```

### Create Test Helper Functions

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_archive() -> TestArchive {
        // Helper to create test data
        TestArchive::new()
            .add_file("test.txt", "content")
            .add_file("data.dat", &[0u8; 1024])
    }

    #[test]
    fn test_with_helper() {
        let archive = create_test_archive();
        // Use archive in test
    }
}
```

### Shared Test Data

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATA: &str = "test content";
    const SMALL_FILE_SIZE: usize = 1024;
    const LARGE_FILE_SIZE: usize = 10 * 1024 * 1024;

    #[test]
    fn test_small_file() {
        // Use TEST_DATA and SMALL_FILE_SIZE
    }
}
```

## Testing Panics

### Use sparingly (panics should be rare)

```rust
#[test]
#[should_panic(expected = "divide by zero")]
fn test_division_by_zero_panics() {
    divide(10, 0);  // This should panic
}
```

**Note:** Prefer `Result` returns over panics in library code.

## Testing Determinism

### Tests MUST Be Deterministic

```rust
// Bad: Non-deterministic (depends on time)
#[test]
fn test_timeout() {
    let start = Instant::now();
    operation();
    assert!(start.elapsed() < Duration::from_secs(1));  // Flaky!
}

// Good: Deterministic (tests behavior, not timing)
#[test]
fn test_operation_completes() {
    let result = operation();
    assert!(result.is_ok());
}
```

### Avoid System Dependencies

```rust
// Bad: Depends on specific file existing
#[test]
fn test_read_config() {
    let config = read_config("/etc/config.toml");
    assert!(config.is_ok());
}

// Good: Uses test fixture
#[test]
fn test_read_config() {
    let temp = NamedTempFile::new().unwrap();
    write_test_config(temp.path());
    let config = read_config(temp.path());
    assert!(config.is_ok());
}
```

## Testing Performance

### Mark Slow Tests

```rust
#[test]
#[ignore]  // Skip by default
fn slow_test_extract_10gb_archive() {
    // This test takes >10 seconds
    let result = extract("huge.zip");
    assert!(result.is_ok());
}
```

Run ignored tests with:
```bash
cargo test -- --include-ignored
```

### Performance Tests Separate

```rust
// Don't mix functional and performance tests
#[test]
fn test_extract_correctness() {
    // Test that extraction works
}

#[test]
#[ignore]
fn bench_extract_performance() {
    // Test that extraction is fast enough
}
```

Use criterion for proper benchmarks instead.

## Test Independence

### Tests MUST NOT Depend on Each Other

```rust
// Bad: Tests share mutable state
static mut COUNTER: i32 = 0;

#[test]
fn test_1() {
    unsafe { COUNTER += 1; }
    assert_eq!(unsafe { COUNTER }, 1);  // Fails if test_2 runs first!
}

#[test]
fn test_2() {
    unsafe { COUNTER += 1; }
    assert_eq!(unsafe { COUNTER }, 2);  // Depends on test_1!
}

// Good: Each test is independent
#[test]
fn test_1() {
    let mut counter = 0;
    counter += 1;
    assert_eq!(counter, 1);
}

#[test]
fn test_2() {
    let mut counter = 0;
    counter += 1;
    assert_eq!(counter, 1);
}
```

## Test Documentation

### Document Non-Obvious Tests

```rust
/// Test that extract_archive correctly handles the freshen mode.
///
/// Freshen mode only updates existing files and does not create new ones.
/// This test verifies that:
/// 1. Existing files are updated if archive version is newer
/// 2. New files in archive are NOT created
#[test]
fn test_extract_freshen_mode() {
    // ...
}
```

### Document Test Setup When Complex

```rust
#[test]
fn test_complex_scenario() {
    // Setup: Create archive with mix of file types
    // - 10 regular files (1KB each)
    // - 5 directories
    // - 3 symlinks
    // - 1 file with special characters in name
    let archive = setup_complex_archive();

    // Test extraction with various options
    // ...
}
```

## Critical Tests for Unzip Project

### extract.rs - Required Tests
```rust
#[test]
fn test_extract_overwrite_mode() { /* Normal overwrite */ }

#[test]
fn test_extract_never_overwrite_mode() { /* Skip existing */ }

#[test]
fn test_extract_freshen_mode() { /* Update existing only */ }

#[test]
fn test_extract_update_mode() { /* Update + create */ }

#[test]
fn test_extract_with_pattern() { /* Include pattern */ }

#[test]
fn test_extract_with_exclude() { /* Exclude pattern */ }

#[test]
fn test_extract_junk_paths() { /* Flatten directory structure */ }

#[test]
fn test_extract_case_insensitive() { /* Case-insensitive matching */ }

#[test]
fn test_extract_large_file_uses_mmap() { /* >1MB file */ }

#[test]
fn test_extract_preserves_timestamps() { /* File modification time */ }

#[test]
fn test_extract_preserves_permissions() { /* Unix permissions */ }
```

### list.rs - Required Tests (Currently Missing!)
```rust
#[test]
fn test_list_contents_short_format() { /* Basic file list */ }

#[test]
fn test_list_contents_verbose_format() { /* With sizes, dates */ }

#[test]
fn test_display_comment() { /* Archive comment */ }

#[test]
fn test_format_large_sizes() { /* GB+ files */ }

#[test]
fn test_format_unicode_filenames() { /* Non-ASCII names */ }
```

### test_archive.rs - Required Tests (Currently Missing!)
```rust
#[test]
fn test_archive_integrity_valid() { /* All CRCs match */ }

#[test]
fn test_archive_integrity_corrupted_crc() { /* CRC mismatch */ }

#[test]
fn test_archive_missing_file() { /* File can't be read */ }

#[test]
fn test_archive_progress_tracking() { /* Progress updates */ }
```

## Test Quality Checklist

Before committing tests:

- [ ] Test names follow naming convention
- [ ] Both success and error paths tested
- [ ] Edge cases covered
- [ ] Tests are independent (can run in any order)
- [ ] Tests are deterministic (no flaky tests)
- [ ] Temporary files use `TempFile`/`TempDir`
- [ ] Test resources are cleaned up
- [ ] Complex tests have documentation
- [ ] No `unwrap()` without `expect()` message
- [ ] Tests complete quickly (< 1 second each)

## Running Tests

```bash
# Run all tests
cargo test

# Run specific module
cargo test glob::tests

# Run with output
cargo test -- --nocapture

# Run ignored tests
cargo test -- --include-ignored

# Run in release mode (faster)
cargo test --release
```

## Coverage Target

Run tests and aim for:
- Overall: > 60%
- Critical paths: > 80%
- Public APIs: 100%

Use `cargo-tarpaulin` if available:
```bash
cargo tarpaulin --out Html
open tarpaulin-report.html
```

## Test Maintenance

- Add tests for every bug fix
- Add tests for every new feature
- Keep tests fast (refactor slow tests)
- Remove obsolete tests
- Update tests when APIs change
- Don't commit failing tests
- Don't ignore failing tests without reason
