---
name: rust-testing
description: Run and write Rust tests with cargo. Use for testing features, fixing test failures, or improving coverage.
allowed-tools: Bash(cargo:*), Bash(make:*), Read, Edit, Grep, Glob
---

# Rust Testing Skill

Comprehensive testing commands and patterns for the unzip project.

## Running Tests

### Basic Test Commands

```bash
# Run all tests
cargo test
make test

# Run specific test by name
cargo test test_glob_match

# Run all tests in a module
cargo test glob::tests

# Run with output (show println!)
cargo test -- --nocapture

# Run with detailed output
cargo test -- --show-output

# Run in release mode
cargo test --release
```

### Advanced Test Commands

```bash
# Run ignored tests
cargo test -- --include-ignored

# Run tests with pattern
cargo test extract -- --nocapture

# Run single test with backtrace
RUST_BACKTRACE=1 cargo test test_name -- --nocapture

# Run tests without capturing output
cargo test -- --nocapture

# Run tests with specific number of threads
cargo test -- --test-threads=1
```

## Test Organization in Unzip Project

### Current Test Locations

```
src/
├── extract.rs      # 9 tests (165 lines)
├── glob.rs         # 10 tests (83 lines)
├── utils.rs        # 7 tests (68 lines)
├── list.rs         # 0 tests ⚠️ NEEDS TESTS
└── test_archive.rs # 0 tests ⚠️ NEEDS TESTS
```

### Unit Tests (in same file)

Located with `#[cfg(test)]` module:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_simple() {
        assert!(glob_match("*.txt", "file.txt"));
    }
}
```

### Integration Tests (separate file)

Located in `tests/` directory:
```rust
// tests/integration_test.rs
use unzip::extract_archive;

#[test]
fn test_full_extraction() {
    // End-to-end test
}
```

## Writing Tests for Unzip

### Test File Extraction

```rust
#[test]
fn test_extract_with_overwrite() {
    use tempfile::TempDir;
    let temp = TempDir::new().unwrap();

    // Setup
    let archive_path = "test_data/sample.zip";
    let output = temp.path();

    // Execute
    let result = extract_to_dir(archive_path, output, /*overwrite=*/true);

    // Verify
    assert!(result.is_ok());
    assert!(output.join("file.txt").exists());
}
```

### Test Glob Pattern Matching

```rust
#[test]
fn test_glob_recursive_wildcard() {
    assert!(glob_match("**/*.rs", "src/main.rs"));
    assert!(glob_match("**/*.rs", "src/lib/mod.rs"));
    assert!(!glob_match("**/*.rs", "README.md"));
}
```

### Test Error Handling

```rust
#[test]
fn test_extract_nonexistent_file() {
    let result = extract_archive("nonexistent.zip");
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.to_string().contains("Failed to open"));
}
```

### Test With Temporary Files

```rust
use tempfile::{NamedTempFile, TempDir};

#[test]
fn test_with_temp_file() {
    let temp_file = NamedTempFile::new().unwrap();
    // ... test with temp_file
    // Automatically cleaned up when dropped
}

#[test]
fn test_with_temp_dir() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    // ... test with temp_dir
    // Automatically cleaned up when dropped
}
```

## Test Patterns and Assertions

### Basic Assertions

```rust
// Equality
assert_eq!(result, expected);

// Inequality
assert_ne!(result, unexpected);

// Boolean
assert!(condition);
assert!(!condition);
```

### Result/Option Testing

```rust
// Test Result is Ok
assert!(result.is_ok());

// Test Result is Err
assert!(result.is_err());

// Test Option is Some
assert!(option.is_some());

// Test Option is None
assert!(option.is_none());

// Extract and test value
let value = result.unwrap();
assert_eq!(value, expected);
```

### Error Message Testing

```rust
#[test]
fn test_error_message() {
    let result = function_that_fails();
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(err_str.contains("expected text"));
}
```

### Testing Panics (use sparingly)

```rust
#[test]
#[should_panic(expected = "overflow")]
fn test_overflow_panics() {
    function_that_panics();
}
```

## Test Coverage Goals for Unzip

### extract.rs (Current: ~80%)
- ✓ Test overwrite modes (normal, overwrite, never)
- ✓ Test freshen mode
- ✓ Test update mode
- ✓ Test pattern matching
- ✓ Test exclude patterns
- ✓ Test junk paths option
- ⚠️ Need: Test large files (>1MB, should trigger mmap)
- ⚠️ Need: Test parallel extraction
- ⚠️ Need: Test error recovery

### glob.rs (Current: ~90%)
- ✓ Test `*` wildcard
- ✓ Test `**` recursive wildcard
- ✓ Test `?` single character
- ✓ Test edge cases
- ✓ Test complex patterns

### list.rs (Current: 0% ⚠️ URGENT)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_contents_short_format() {
        // TODO: Create test archive and verify output
    }

    #[test]
    fn test_list_contents_verbose_format() {
        // TODO: Verify size, date, compression ratio
    }

    #[test]
    fn test_display_comment() {
        // TODO: Test with archive that has comment
    }

    #[test]
    fn test_format_with_large_sizes() {
        // TODO: Test size formatting (KB, MB, GB)
    }
}
```

### test_archive.rs (Current: 0% ⚠️ URGENT)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_integrity_valid() {
        // TODO: Test with valid archive
    }

    #[test]
    fn test_archive_integrity_corrupted_crc() {
        // TODO: Test with corrupted CRC
    }

    #[test]
    fn test_archive_progress_tracking() {
        // TODO: Verify progress updates
    }
}
```

### utils.rs (Current: ~80%)
- ✓ Test file filtering
- ✓ Test size formatting
- ✓ Test date conversion
- ⚠️ Need: Test edge cases for date conversion

## Running Specific Test Suites

```bash
# Run tests in extract module
cargo test extract::tests

# Run tests in glob module
cargo test glob::tests

# Run all tests matching pattern
cargo test overwrite

# Run tests in specific file
cargo test --test integration_test
```

## Test Performance

### Running Tests Quickly

```bash
# Run tests in parallel (default)
cargo test

# Run tests sequentially (for debugging)
cargo test -- --test-threads=1

# Skip slow tests
cargo test --lib  # Only library tests, skip integration tests
```

### Marking Slow Tests

```rust
#[test]
#[ignore]  // Skip by default
fn slow_test_large_archive() {
    // This test takes >5 seconds
}

// Run with:
cargo test -- --include-ignored
```

## Debugging Failing Tests

### Get More Information

```bash
# Show stdout/stderr
cargo test -- --nocapture

# Show backtrace
RUST_BACKTRACE=1 cargo test

# Full backtrace
RUST_BACKTRACE=full cargo test

# Run single test with all output
RUST_BACKTRACE=1 cargo test specific_test -- --nocapture --exact
```

### Common Test Failures

#### Assertion Failed
```
assertion `left == right` failed
  left: "actual"
 right: "expected"
```

**Fix**: Check why values differ

#### Test Panicked
```
thread 'test_name' panicked at 'called `Result::unwrap()` on an `Err` value'
```

**Fix**: Use `?` operator or handle error properly

#### Resource Cleanup
```
error: Directory not empty
```

**Fix**: Use `TempDir` for automatic cleanup

## Test Best Practices

### Do's ✓

- Write tests for public APIs
- Test both success and error paths
- Use descriptive test names
- Keep tests independent
- Clean up resources
- Use `TempDir`/`TempFile` for file operations
- Test edge cases (empty, max, boundary)

### Don'ts ✗

- Don't use unwrap() in tests (use `?` or `expect()`)
- Don't depend on test execution order
- Don't share mutable state between tests
- Don't create files in source directory
- Don't write tests that take >1 second
- Don't ignore failing tests

## Test Documentation

### Document Test Intent

```rust
/// Test that extract_archive correctly handles overwrite mode.
///
/// This test verifies that when overwrite=true is set, existing files
/// are replaced without prompting the user.
#[test]
fn test_extract_with_overwrite_mode() {
    // ...
}
```

### Use Clear Test Names

```
test_<function>_<condition>_<expected_result>
```

Examples:
- `test_glob_match_wildcard_succeeds`
- `test_glob_match_no_match_fails`
- `test_extract_nonexistent_file_returns_error`
- `test_extract_with_overwrite_replaces_files`

## Integration with Makefile

```bash
# Run all tests
make test

# Run with output
make test ARGS='-- --nocapture'

# Run specific test
make test ARGS='test_glob_match'
```

## Coverage Analysis (Optional)

If `cargo-tarpaulin` is installed:

```bash
# Install
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --out Html

# View report
open tarpaulin-report.html
```

## Quick Reference

| Command | Purpose |
|---------|---------|
| `cargo test` | Run all tests |
| `cargo test <name>` | Run specific test |
| `cargo test -- --nocapture` | Show output |
| `cargo test -- --show-output` | Show output for passing tests too |
| `RUST_BACKTRACE=1 cargo test` | Show backtrace on panic |
| `cargo test --release` | Run in release mode |
| `cargo test -- --test-threads=1` | Run sequentially |
| `cargo test -- --include-ignored` | Run ignored tests |

## Testing Checklist

Before committing:

- [ ] All tests pass: `cargo test`
- [ ] Tests added for new functionality
- [ ] Tests added for bug fixes
- [ ] Edge cases covered
- [ ] Error paths tested
- [ ] No ignored failing tests
- [ ] Tests are deterministic
- [ ] Tests run in reasonable time (<30s total)

Use this skill to run tests, write new tests, or debug test failures.
