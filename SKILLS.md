# SKILLS.md

This file defines **Codex skills** for the `unzip` project.
Skills describe *what kinds of changes are allowed*, *where they apply*,
and *which constraints must be respected*.

All skills assume:
- Rust stable toolchain
- No breaking CLI or output changes unless explicitly stated
- Info-ZIP compatibility is a primary goal

---

## Skill: rust_refactor

**Purpose**  
Refactor code for clarity and maintainability without changing behavior.

**Scope**  
- `src/**/*.rs`

**Rules**
- No observable behavior changes
- No CLI or output changes
- Preserve Info-ZIP compatibility
- No new allocations in hot paths
- Prefer explicit control flow over abstraction

**Validation**
- `cargo check`
- `cargo test`

---

## Skill: rust_bugfix

**Purpose**  
Fix correctness, logic, or safety bugs.

**Scope**  
- `src/**/*.rs`

**Rules**
- Minimal, targeted diffs
- No new `unwrap()` or `expect()` in production code
- Do not relax validation or error handling
- Preserve existing semantics

**Requirements**
- Add a regression test when feasible
- Comment on non-obvious root causes

**Validation**
- `cargo test`
- `cargo clippy` (no new warnings)

---

## Skill: rust_perf

**Purpose**  
Optimize performance-critical paths.

**Scope**
- `src/extract.rs`
- `src/utils.rs`
- `src/linux.rs`
- `src/main.rs`
- `benches/extraction_bench.rs`

**Rules**
- Behavior must remain identical
- Avoid increasing memory usage
- Avoid new syscalls in hot paths
- Prefer stack allocation
- Respect mmap threshold logic (>1MB)

**Platform Constraints**
- Linux-only optimizations must be behind `cfg(target_os = "linux")`
- Non-Linux platforms must remain fully functional

**Validation**
- `cargo bench`
- `cargo test`

---

## Skill: unzip_cli_compat

**Purpose**  
Add or modify CLI behavior while preserving Info-ZIP compatibility.

**Scope**
- `src/args.rs`
- `src/main.rs`
- `src/list.rs`
- `src/zipinfo.rs`

**Rules**
- Flag names and semantics must match Info-ZIP
- Output should be byte-for-byte compatible when possible
- Help text must reflect Info-ZIP behavior
- No breaking changes to existing flags

**Validation**
- Manual comparison with Info-ZIP `unzip` when possible
- `cargo test`

---

## Skill: rust_tests

**Purpose**  
Add or improve test coverage.

**Scope**
- `src/**/*.rs`
- `tests/` (if present)

**Rules**
- Prefer unit tests within modules
- No flaky or timing-dependent tests
- Tests must be deterministic
- Use temp directories for filesystem tests

**Focus Areas**
- Corrupted ZIPs
- Encrypted archives
- CRC mismatches
- Include/exclude glob behavior
- Timestamp and permission handling

**Validation**
- `cargo test`

---

## Skill: rust_security

**Purpose**  
Harden the codebase against malformed or malicious ZIP files.

**Scope**
- ZIP parsing
- Extraction paths
- Password handling

**Rules**
- Never trust ZIP metadata blindly
- Prevent path traversal (`../`, absolute paths, drive prefixes)
- Avoid integer overflows and unchecked casts
- Do not weaken encryption or password checks

**Validation**
- `cargo test`
- Manual review of affected logic

---

## Skill: rust_docs

**Purpose**  
Improve documentation and internal clarity.

**Scope**
- `src/**/*.rs`
- `README.md`

**Rules**
- No behavior changes
- Comments should explain *why*, not *what*
- Document invariants and assumptions

---

## Skill: rust_style

**Purpose**  
Enforce consistent Rust style.

**Scope**
- Entire repository

**Rules**
- Follow `rustfmt` defaults
- Keep code readable and explicit
- Consistent error-handling patterns

**Validation**
- `cargo fmt --check`
- `cargo clippy`

---

## Skill: contributor_safe_change

**Purpose**  
Allow small, safe changes suitable for external contributors.

**Scope**
- Tests
- Documentation
- Non-critical code paths

**Rules**
- No changes to extraction logic
- No performance-sensitive changes
- No CLI behavior changes
- Changes must be easy to review

**Validation**
- `cargo test`
- `cargo check`

---

## Skill Usage

When invoking Codex, explicitly reference the skill:

Examples:
- “Use **rust_perf** to optimize large-file extraction.”
- “Apply **rust_bugfix** to fix CRC validation in test mode.”
- “Using **unzip_cli_compat**, add Info-ZIP compatible flag behavior.”

Codex must follow the selected skill’s constraints.

