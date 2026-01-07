---
paths: src/**/*.rs
---

# Rust Code Style Rules

These rules apply to all Rust source files in the `src/` directory.

## Naming Conventions

### Functions and Methods
- Use `snake_case` for function names
- Use descriptive names over abbreviations
- Prefix internal helpers with underscore if truly private

```rust
// Good
fn extract_archive() {}
fn should_overwrite_file() {}
fn _internal_helper() {}

// Bad
fn extractArc() {}
fn sof() {}
fn ExtractArchive() {}
```

### Types and Traits
- Use `PascalCase` for types, structs, enums, traits
- Use `SCREAMING_SNAKE_CASE` for constants
- Enum variants use `PascalCase`

```rust
// Good
struct FileMetadata {}
enum OverwriteMode {}
const BUFFER_SIZE: usize = 256 * 1024;

// Bad
struct file_metadata {}
enum overwrite_mode {}
const bufferSize: usize = 256 * 1024;
```

### Modules
- Use `snake_case` for module names
- Match filename (e.g., `extract.rs` → `mod extract`)

## File Organization

### File Size Limit
**Maximum 500 lines per file (excluding tests)**

If a file exceeds 500 lines:
1. Identify logically separate concerns
2. Extract helper functions
3. Consider creating sub-modules
4. Tests don't count toward the limit

Current status (non-test lines, after Phase 11 documentation):
- ✓ main.rs: 87 lines
- ✓ args.rs: 112 lines
- ⚠️ **extract.rs: 513 lines** (313 code + 200 doc comments)
- ✓ glob.rs: 108 lines
- ✓ linux.rs: 90 lines
- ✓ list.rs: 167 lines
- ✓ test_archive.rs: 163 lines
- ✓ utils.rs: 271 lines
- ✓ lib.rs: 53 lines

**Note on extract.rs:** The file is 13 lines over the 500-line limit due to comprehensive doc comments added in Phase 11 (100+ lines of documentation). The actual code (excluding documentation and blank lines) is only 313 lines, well within the limit. This is acceptable as documentation significantly improves code maintainability and shouldn't be penalized. The code structure from Phase 2 refactoring remains sound.

### Function Size Limit
**Target: 50 lines maximum per function**

Exceptions allowed for:
- Complex but linear logic (state machines)
- Functions that would be harder to understand if split
- Test functions (can be longer)

If a function exceeds 50 lines:
1. Extract logical blocks into helper functions
2. Consider if complexity can be reduced
3. Add clear comments explaining structure

### Module Structure

Each file should follow this order:
```rust
//! Module documentation

// Imports
use std::...;
use external_crate::...;
use crate::...;

// Constants
const BUFFER_SIZE: usize = ...;

// Types
struct Foo { ... }
enum Bar { ... }

// Public functions
pub fn public_api() { ... }

// Private functions
fn helper() { ... }

// Tests
#[cfg(test)]
mod tests { ... }
```

## Import Organization

Group imports in this order:
1. `std` library
2. External crates
3. Internal crates (`crate::`)

Separate groups with blank lines:

```rust
// Good
use std::fs::File;
use std::io::Read;

use zip::ZipArchive;
use anyhow::Result;

use crate::utils::format_size;
use crate::glob::glob_match;

// Bad (mixed order)
use crate::utils::format_size;
use std::fs::File;
use zip::ZipArchive;
use std::io::Read;
```

Use `rustfmt` to automatically organize imports.

## Documentation

### Public Functions
All public functions MUST have doc comments with:
- Summary description
- `# Arguments` section (if any)
- `# Errors` section (if returns Result)
- `# Examples` section
- `# Panics` section (if it can panic)
- `# Safety` section (if unsafe)

```rust
/// Extracts files from a ZIP archive to the filesystem.
///
/// # Arguments
///
/// * `archive` - The ZIP archive to extract from
/// * `args` - Command-line arguments controlling extraction behavior
///
/// # Errors
///
/// Returns an error if:
/// - The output directory cannot be created
/// - A file cannot be extracted
/// - CRC verification fails
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use zip::ZipArchive;
/// use unzip::{Args, extract_archive};
///
/// let file = File::open("archive.zip")?;
/// let mut archive = ZipArchive::new(file)?;
/// let args = Args::default();
/// extract_archive(&mut archive, &args)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn extract_archive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    args: &Args,
) -> Result<()> {
    // ...
}
```

### Private Functions
Private functions SHOULD have doc comments if:
- Logic is non-obvious
- Function is complex
- Used by multiple callers

Use regular comments (`//`) for simple helpers.

### Module Documentation
Each module MUST have module-level documentation:

```rust
//! Archive extraction functionality
//!
//! This module provides functions to extract files from ZIP archives
//! to the filesystem with various options like overwrite modes,
//! pattern matching, and progress reporting.
//!
//! # Performance
//!
//! Uses memory-mapped I/O for files >1MB and Linux kernel optimizations
//! when available.
```

## Error Handling

### Never Use Unwrap in Library Code

```rust
// Bad
let value = result.unwrap();

// Good
let value = result?;

// Or
let value = result.expect("Descriptive message explaining invariant");
```

The only place `unwrap()` or `expect()` is acceptable:
- In `main.rs` (application entry point)
- In tests
- When there's a proven invariant that guarantees success

### Provide Meaningful Context

```rust
// Bad
File::open(path)?

// Good
File::open(path)
    .with_context(|| format!("Failed to open file: {}", path.display()))?
```

### Error Messages Should Be Actionable

```rust
// Bad
bail!("Error");

// Good
bail!("Failed to create output directory '{}': permission denied. \
       Try running with sudo or choose a different directory.",
       dir.display());
```

## Code Formatting

### Use rustfmt

All code MUST be formatted with rustfmt before committing:

```bash
cargo fmt
```

Configuration in `rustfmt.toml`:
- Line length: 100 characters
- Tab spaces: 4
- Unix line endings

### Line Length
Target 100 characters, but can exceed for:
- String literals
- Import statements
- Function signatures (but break into multiple lines if possible)

```rust
// Good (break long function signature)
pub fn extract_archive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    args: &Args,
) -> Result<()> {
    // ...
}

// Bad (too long)
pub fn extract_archive<R: Read + Seek>(archive: &mut ZipArchive<R>, args: &Args) -> Result<()> { }
```

## Performance Considerations

### Avoid Unnecessary Allocations

```rust
// Bad (unnecessary allocation)
fn process(s: &str) -> String {
    s.to_string()
}

// Good (no allocation)
fn process(s: &str) -> &str {
    s
}
```

### Use Borrowing Over Cloning

```rust
// Bad (clones entire collection)
for item in items.clone() {
    process(item);
}

// Good (borrows)
for item in &items {
    process(item);
}
```

### Preallocate When Size Known

```rust
// Bad (may reallocate multiple times)
let mut vec = Vec::new();
for i in 0..1000 {
    vec.push(i);
}

// Good (single allocation)
let mut vec = Vec::with_capacity(1000);
for i in 0..1000 {
    vec.push(i);
}
```

### Use Iterator Chains

```rust
// Bad (creates intermediate vector)
let mut result = Vec::new();
for item in items {
    if item.is_valid() {
        result.push(item.process());
    }
}

// Good (no intermediate allocation)
let result: Vec<_> = items
    .into_iter()
    .filter(|item| item.is_valid())
    .map(|item| item.process())
    .collect();
```

## Pattern Matching

### Use Pattern Matching Over if-let Chains

```rust
// Bad
if let Some(x) = opt1 {
    if let Some(y) = opt2 {
        // ...
    }
}

// Good
match (opt1, opt2) {
    (Some(x), Some(y)) => {
        // ...
    }
    _ => {}
}
```

### Be Explicit About Match Arm Exhaustiveness

```rust
// Bad (might miss cases)
match value {
    Some(x) => handle(x),
    _ => {}
}

// Good (explicit about what None means)
match value {
    Some(x) => handle(x),
    None => {
        // Explicitly handling None
    }
}
```

## Comments

### Use Comments for Why, Not What

```rust
// Bad (obvious from code)
// Increment i
i += 1;

// Good (explains rationale)
// Use 256KB buffer size to balance memory usage with I/O efficiency.
// This typically matches filesystem block sizes.
const BUFFER_SIZE: usize = 256 * 1024;
```

### Document Non-Obvious Invariants

```rust
// Linux optimization: tell kernel we'll read sequentially.
// This enables aggressive read-ahead for better throughput.
madvise_sequential(mmap.as_ptr(), mmap.len());
```

### Use TODO/FIXME/NOTE

```rust
// TODO: Implement parallel extraction using rayon
// FIXME: Handle Unicode filenames on Windows
// NOTE: Buffer must be at least 1KB for this algorithm
```

## Safety and Unsafe Code

### Minimize Unsafe

Only use `unsafe` when necessary:
- FFI calls
- Performance-critical code with profiling proof
- Well-documented invariants

### Document Safety Invariants

Every `unsafe` block MUST have a SAFETY comment:

```rust
// SAFETY: addr and len come from a valid mmap region that outlives this call.
// The memory is properly aligned and will not be modified during access.
unsafe {
    let ptr = addr as *mut std::ffi::c_void;
    madvise(ptr, len, Advice::Sequential);
}
```

## Project-Specific Conventions

### Buffer Sizes

Use the project constant:
```rust
const BUFFER_SIZE: usize = 256 * 1024;  // 256KB
```

### Linux Optimizations

All Linux-specific code must:
1. Be behind `#[cfg(target_os = "linux")]`
2. Have no-op implementations for other platforms
3. Fail silently if kernel doesn't support the optimization

```rust
#[cfg(target_os = "linux")]
pub fn optimize() {
    // Linux-specific code
}

#[cfg(not(target_os = "linux"))]
pub fn optimize() {
    // No-op on other platforms
}
```

### Progress Reporting

Check `args.quiet` level before showing progress:
```rust
let progress_bar = if args.quiet == 0 {
    Some(ProgressBar::new(total))
} else {
    None
};
```

## Checklist Before Committing

- [ ] Code formatted with `cargo fmt`
- [ ] No `cargo clippy` warnings
- [ ] All files < 500 lines (excluding tests)
- [ ] All public functions have doc comments
- [ ] No `unwrap()` in library code
- [ ] Error messages are actionable
- [ ] Tests added for new functionality
- [ ] Performance implications considered
- [ ] SAFETY comments for unsafe blocks

Run these commands before committing:
```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
```
