---
name: reviewing-rust-testing
description: Use when reviewing Rust unit or integration tests, test helpers, or code-review coverage reports to verify googletest matchers, fallible Result setup, RAII resource cleanup, and coverage of public APIs and error paths.
---

# Reviewing Rust Testing

Tests must be expressive, non-panicking, isolated, and thorough across both happy paths and failure branches. Full contracts: [SPEC.md](./SPEC.md).

## Mechanical Pre-Checks

Run before inspecting test bodies:

1. `cargo test --all-targets --all-features`
2. `cargo run -p code-review -- purist` — enforces `googletest_conventions`, `test_patterns`, `no_test_prefix`, `test_matcher_borrow_simplification`, `raii_temp_directories`, and `no_unsafe_in_tests`.
3. `cargo run -p code-review -- coverage` — inspects line, function, branch, and region coverage and flags untested files/paths.

## Review Checklist

1. **GoogleTest Harness & Expressive Matchers**
   - Tests use `#[googletest::test]` (or `#[gtest]`) and `expect_that!` / `assert_that!` with direct matchers (`eq(...)`, `contains_substring(...)`, `ok(...)`, `err(...)`)—never `#[test]`, `assert!`, `assert_eq!`, `assert_ne!`, or `expect_that!(a == b, is_true())`.
2. **Descriptive Naming & Fallible Propagation**
   - Function names follow `<verb>_<scenario>_<outcome>` (e.g., `parse_invalid_json_returns_syntax_error`) with no `test_` prefix.
   - Test bodies return `Result<(), Box<dyn std::error::Error>>` (or `googletest::Result<()>`) and propagate setup errors via `?` or `.or_fail()?`—never `.unwrap()` or `.expect()`.
3. **Deterministic Isolation & Zero Leaks (`leaky_tests`)**
   - Temp files use RAII `tempfile::tempdir()?`—never fixed `/tmp/...` paths, workspace mutations, `unsafe { env::set_var(...) }`, `env::set_current_dir`, network calls, or timing sleeps.
4. **Public API & Error-Path Coverage (`coverage_gaps`)**
   - Every new public API, CLI flag, parser branch, exit code (`0`/`1`/`2`), and `Error` variant (missing file, malformed input, threshold failure) has dedicated tests; reject assertion-free or happy-path-only suites.

## Subagent Prompt & Output Format

```text
Review Rust tests and coverage in <target_path_or_diff> using skills/reviewing-rust-testing/SKILL.md.
Run `cargo test`, `code-review purist`, and `code-review coverage` (if llvm-cov is installed), and return:
Verdict: PASS | FAIL
Findings (empty if clean—no speculative nitpicks):
- [CRITICAL|MAJOR|MINOR] <file>:<line> — testing::<rule>: <issue> | Fix: <remediation>
```

## Common Rationalizations

| Excuse                                                      | Reality                                                                                   |
| ----------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| "`.unwrap()` in a test just panics on failure anyway."      | Panics abort early with poor diffs and skip context; return `Result` with `?`/`or_fail`.  |
| "`std::fs::create_dir("/tmp/test-x")` is quick."            | Collides under parallel tests and leaks on failure; use `tempfile::tempdir()?`.           |
| "85% overall coverage is high enough, skip error branches." | Uncovered error paths hide production bugs; test every `Error` variant and boundary case. |
| "`expect_that!(res.is_ok(), is_true())` verifies success."  | Erases `Err` diagnostics and ignores the `Ok` payload; match `ok(eq(...))` or use `?`.    |

## Red Flags — STOP

- `#[test]`, `assert_eq!`, `test_*` prefixes, `.unwrap()`, or `.expect()` inside test modules.
- Manual `/tmp` paths, `set_current_dir`, `unsafe` env mutation, or order-dependent shared state.
- New `Error` variants, parser branches, or CLI exit codes with zero negative tests.
