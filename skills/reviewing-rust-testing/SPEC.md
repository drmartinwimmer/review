# Skill Specification: Rust Testing & Coverage Review (`reviewing-rust-testing`)

## 1. Purpose and Scope

The `reviewing-rust-testing` skill (`skills/reviewing-rust-testing/SKILL.md`) audits unit and integration test suites for `googletest` conventions, expressive failure diagnostics, fallible test propagation, deterministic RAII resource cleanup, and sufficient source-based code coverage across public APIs and error paths.

- **In Scope:** `#[googletest::test]` (`#[gtest]`) usage, `expect_that!` / `assert_that!` matcher quality, `<verb>_<description>_<outcome>` naming without `test_` prefixes, fallible `Result` returns (`?` / `.or_fail()?`) instead of `.unwrap()`/`.expect()`, RAII `tempfile::tempdir()` isolation without CWD/env mutation leaks, and line/branch/error-path coverage verified via `code-review coverage`.
- **Out of Scope:** Production module architecture (`reviewing-rust-modularity`) and `Cargo.toml` lint preset definitions (`reviewing-rust-lint-hygiene`).

---

## 2. Mechanical Tool Synergy

1. `cargo test --all-targets --all-features`: Verifies all unit, binary, doc, and integration tests pass deterministically under parallel execution.
2. `cargo run -p code-review -- purist`: Enforces:
   - `purist::googletest_conventions` (`#[googletest::test]`, `expect_that!` / `assert_that!` over `assert!`/`assert_eq!`/`assert_ne!`, no `.expect(...)` in tests, direct matchers `eq(y)`/`ok(...)` over `assert_that!(x == y, is_true())`)
   - `purist::test_patterns` (`<verb>_<description>_<outcome>` naming, no `.unwrap()` in test bodies)
   - `purist::no_test_prefix` (forbids redundant `test_` prefix)
   - `purist::test_matcher_borrow_simplification`
   - `purist::raii_temp_directories` (requires `tempfile::TempDir` over manual `/tmp` paths)
   - `purist::no_unsafe_in_tests` (forbids `unsafe`, e.g., `std::env::set_var` data races across parallel test threads)
3. `cargo run -p code-review -- coverage` (wrapping `cargo-llvm-cov`): Measures line, function, branch, and region coverage and highlights untested files and uncovered error branches (`coverage_gaps`).

---

## 3. Review Invariants & Domain Criteria

1. **GoogleTest Harness & Expressive Matchers:**
   - Every test uses `#[googletest::test]` (or `#[gtest]`), returns `Result<(), Box<dyn std::error::Error>>` or `googletest::Result<()>`, and asserts with `expect_that!` (preferred for multi-check reporting) or `assert_that!` using structural matchers (`eq`, `contains_substring`, `err(...)`, `ok(...)`, `len(eq(...))`).
   - Reject tautological tests (tests with zero assertions or assertions that only check `is_ok()` without inspecting the returned value).
2. **Fallible Setup & Descriptive Naming:**
   - Test names follow `<verb>_<scenario>_<expected_outcome>` (e.g., `parse_malformed_toml_returns_parse_error`), never `test_foo`.
   - Setup steps propagate errors with `?` or `.or_fail()?` so failures report clean context and run `Drop` guards cleanly.
3. **Deterministic Isolation & Zero Resource Leaks (`leaky_tests`):**
   - Filesystem tests use `tempfile::tempdir()?` (never writing into workspace paths, fixed `/tmp/...` paths, or mutating process-global `std::env::set_current_dir` / `set_var`).
   - Tests must not depend on network access, wall-clock sleeps, or execution ordering.
4. **Public API & Error-Path Coverage (`coverage_gaps`):**
   - Every new public function, CLI flag/subcommand, parser branch, and `Error` enum variant (missing path, malformed input, threshold failure, non-zero exit code) must have dedicated negative and boundary tests—not just a single happy-path smoke test.

### False-Positive Exclusions

- Thin subprocess wrappers around external binaries absent in isolated test sandboxes should separate command construction + output parsing (unit-tested thoroughly) from raw process spawning.

---

## 4. Subagent Dispatch & Finding Contract

- **Inputs:** Target crate/diff, `cargo test` & `code-review purist` results, and `code-review coverage --json` summary (when `cargo-llvm-cov` is available).
- **Output Schema:**
  - `verdict`: `"PASS"` | `"FAIL"`
  - `findings`: List of `{ severity: "CRITICAL" | "MAJOR" | "MINOR", file_span: "<path>:<line>", category: "testing::<conventions|leaky_resource|weak_assertion|coverage_gap>", issue: "<test flaw or untested path>", fix: "<concrete test addition or rewrite>" }`. Empty when compliant.

---

## 5. Playbook Hygiene & Verification

- `skills/reviewing-rust-testing/SKILL.md` starts with YAML frontmatter (`name: reviewing-rust-testing`, `description: Use when...`), stays strictly `< 500` words (`wc -w`), and includes checklist, subagent prompt, rationalization table, and red flags.
- Verified via `prettier --check "**/*.{md,json}"` and `cargo run -p code-review -- check`.
