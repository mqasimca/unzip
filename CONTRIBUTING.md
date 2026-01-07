# Contributing to Rust Unzip

Thank you for your interest in contributing! This document provides guidelines and instructions for contributing to this project.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Environment](#development-environment)
- [Code Standards](#code-standards)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Release Process](#release-process)

## Getting Started

### Prerequisites

- Rust 1.82 or later (Edition 2024)
- Git
- Make (optional, for convenience commands)
- Linux (optional, for testing kernel optimizations)

### Initial Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/unzip.git
cd unzip

# Build the project
cargo build

# Run tests to verify setup
cargo test

# Build optimized release version
cargo build --release
```

## Development Environment

### Recommended Tools

- **rustfmt** - Code formatting (required)
- **clippy** - Linting (required)
- **rust-analyzer** - IDE support
- **cargo-watch** - Auto-rebuild on changes

### Editor Configuration

This project uses:
- `rustfmt.toml` for code formatting rules
- `clippy.toml` for linting configuration
- `.claude/` for AI-assisted development settings

## Code Standards

### File Organization

- **Maximum 500 lines per file** (excluding tests)
- **Target 50 lines per function** (guidelines, not strict)
- Use sub-modules or helper functions for large files

### Naming Conventions

- **Functions**: `snake_case` (e.g., `extract_archive`)
- **Types/Structs**: `PascalCase` (e.g., `OverwriteDecision`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `BUFFER_SIZE`)
- **Modules**: `snake_case` matching filename

### Error Handling

**Never use `.unwrap()` in library code**. Always use:

```rust
// Good
let value = result?;

// Good with context
let value = result
    .with_context(|| format!("Failed to process {}", name))?;

// Bad
let value = result.unwrap();

// Only in main.rs or tests
let value = result.expect("Config must be valid");
```

### Documentation

All public functions must have doc comments with:

```rust
/// Brief description of function
///
/// # Arguments
///
/// * `param` - Description of parameter
///
/// # Errors
///
/// Returns an error if:
/// - Condition 1
/// - Condition 2
///
/// # Examples
///
/// ```
/// use unzip::function_name;
///
/// let result = function_name()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn function_name() -> Result<()> {
    // ...
}
```

### Performance Guidelines

- Avoid unnecessary allocations
- Prefer borrowing over cloning
- Preallocate buffers when size is known
- Use iterator chains instead of explicit loops
- Profile before optimizing

### Platform-Specific Code

All Linux-specific code must:

```rust
#[cfg(target_os = "linux")]
pub fn optimize() {
    // Linux-specific implementation
}

#[cfg(not(target_os = "linux"))]
pub fn optimize() {
    // No-op for other platforms
}
```

## Testing

### Test Requirements

- **Minimum 60% overall test coverage**
- **Module-specific targets:**
  - `extract.rs`: 80%+
  - `glob.rs`: 90%+
  - `utils.rs`: 80%+
  - `list.rs`: 70%+
  - `test_archive.rs`: 70%+

### Running Tests

```bash
# Run all tests
cargo test

# Run specific module
cargo test glob::tests

# Run with output
cargo test -- --nocapture

# Run in release mode
cargo test --release

# Using Makefile
make test
```

### Writing Tests

Test naming: `test_<function>_<condition>_<expected_result>`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_glob_match_wildcard_succeeds() {
        assert!(glob_match("*.txt", "file.txt"));
    }

    #[test]
    fn test_glob_match_no_match_fails() {
        assert!(!glob_match("*.txt", "file.rs"));
    }
}
```

### Test Guidelines

- Tests must be deterministic (no flaky tests)
- Use `TempDir`/`TempFile` for filesystem operations
- Tests should complete quickly (<1 second each)
- Test both success and error paths
- Cover edge cases and boundary conditions

## Submitting Changes

### Before Committing

Run the pre-commit checklist:

```bash
# Format code
cargo fmt

# Run linter (must pass with no warnings)
cargo clippy -- -D warnings

# Run tests
cargo test

# Build release version
cargo build --release

# Or use Makefile
make ci-full
```

### Commit Message Format

```
<type>: <short summary>

<detailed description if needed>

<optional footer>
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `refactor`: Code refactoring
- `docs`: Documentation changes
- `test`: Adding or updating tests
- `perf`: Performance improvements
- `chore`: Build process or auxiliary tool changes

Examples:
```
feat: Add parallel extraction support

Implements multi-threaded extraction using rayon for improved
performance on multi-core systems.

fix: Handle Unicode filenames correctly on Windows

Converts UTF-8 filenames to Windows-compatible encoding to prevent
extraction failures with non-ASCII characters.
```

### Pull Request Process

1. **Create a feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Follow code standards
   - Add tests for new functionality
   - Update documentation

3. **Run quality checks**
   ```bash
   make ci-full
   ```

4. **Commit your changes**
   ```bash
   git add .
   git commit -m "feat: your feature description"
   ```

5. **Push to your fork**
   ```bash
   git push origin feature/your-feature-name
   ```

6. **Create Pull Request**
   - Provide clear description
   - Reference any related issues
   - Ensure CI passes
   - Wait for review

### Pull Request Checklist

- [ ] Code follows project style guidelines
- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] New code has tests
- [ ] Documentation is updated
- [ ] Commit messages are clear and descriptive
- [ ] No files exceed 500 lines (excluding tests)
- [ ] Public functions have doc comments

## Code Review

### What Reviewers Look For

- **Correctness**: Does the code work as intended?
- **Tests**: Are there adequate tests?
- **Performance**: Any unnecessary allocations or inefficiencies?
- **Safety**: Proper error handling, no unwrap() in library code?
- **Documentation**: Clear comments and doc strings?
- **Style**: Follows project conventions?

### Responding to Feedback

- Address all review comments
- Ask for clarification if needed
- Update PR with requested changes
- Mark conversations as resolved when addressed

## Common Issues

### Build Failures

```bash
# Clear build cache
cargo clean

# Rebuild
cargo build
```

### Test Failures

```bash
# Run specific failing test with output
cargo test test_name -- --nocapture

# Check for race conditions
cargo test -- --test-threads=1
```

### Formatting Issues

```bash
# Auto-format all code
cargo fmt

# Check formatting without changing files
cargo fmt -- --check
```

### Clippy Warnings

```bash
# Show all warnings
cargo clippy

# Apply automatic fixes
cargo clippy --fix
```

## Performance Testing

### Benchmarking

```bash
# Create test archive
dd if=/dev/zero of=test.dat bs=10M count=10
zip test.zip test.dat

# Time extraction
time ./target/release/unzip -q -o -d /tmp/out test.zip

# Compare with Info-ZIP
time /usr/bin/unzip -q -o -d /tmp/out test.zip
```

### Profiling

```bash
# Install cargo-flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --bin unzip -- test.zip
```

## Release Process

Releases are automated via GitHub Actions:

1. Update version in `Cargo.toml`
2. Create git tag: `git tag v0.x.0`
3. Push tag: `git push origin v0.x.0`
4. Create GitHub release from tag
5. CI builds binaries for all platforms
6. Binaries uploaded to release automatically

## Getting Help

- Open an issue for bugs or feature requests
- Check existing issues before creating new ones
- Provide minimal reproducible examples for bugs
- Include system information (OS, Rust version)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

## Code of Conduct

### Our Standards

- Be respectful and inclusive
- Welcome newcomers
- Focus on what is best for the community
- Show empathy towards others
- Accept constructive criticism gracefully

### Unacceptable Behavior

- Harassment or discriminatory language
- Personal attacks
- Trolling or inflammatory comments
- Publishing others' private information

### Enforcement

Project maintainers are responsible for clarifying standards and will take appropriate action in response to unacceptable behavior.

---

Thank you for contributing to Rust Unzip! 🦀
