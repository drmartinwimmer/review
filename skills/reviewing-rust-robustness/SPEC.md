# Skill Specification: Rust Robustness & Error Handling Review (`reviewing-rust-robustness`)

## 1. Purpose and Scope

The `reviewing-rust-robustness` skill (`skills/reviewing-rust-robustness/SKILL.md`) audits production Rust code for panic freedom, structured fallible error modeling, silent-failure prevention, memory/numeric safety, and graceful edge-case handling.

- **In Scope:** Absence of `.unwrap()`, `.expect()`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, and unchecked indexing/string slicing in production code; structured `thiserror` error enums over `String` or `Box<dyn Error>`; non-ignored `Result`/`Future`/`#[must_use]` values; checked numeric casts and time arithmetic; and `unsafe` block soundness/avoidance.
- **Out of Scope:** Test-only assertion style (`reviewing-rust-testing`), lint suppression syntax (`reviewing-rust-lint-hygiene`), and filesystem sandbox path anchoring (`reviewing-containment-safety`).

---

## 2. Mechanical Tool Synergy

1. `CARGO_BUILD_WARNINGS=deny cargo clippy --all-targets --all-features -- -D warnings`: Enforces the `Strict` profile (`crates/configure-lints/src/presets.rs`), including:
   - **Don't Panic:** `unwrap_used`, `indexing_slicing`, `string_slice`, `panic`, `todo`, `unimplemented`, `unreachable`, `get_unwrap`, `unwrap_in_result`, `unchecked_time_subtraction`, `panic_in_result_fn`.
   - **Don't Fail Silently:** `let_underscore_future`, `let_underscore_must_use`, `unused_result_ok`, `map_err_ignore`, `ignored_unit_patterns`.
   - **Memory & Numeric Safety:** `mem_forget`, `undocumented_unsafe_blocks`, `multiple_unsafe_ops_per_block`, `unsafe_op_in_unsafe_fn`, `large_stack_arrays`, `float_cmp`, `cast_sign_loss`, `cast_possible_truncation`, `cast_possible_wrap`, `cast_precision_loss`, `checked_conversions`.
   - **Process Hygiene:** `exit`, `dbg_macro`, `rc_mutex`, `infinite_loop`.
2. `cargo run -p code-review -- purist`: Enforces `purist::error_types` (no `Result<T, String>`/`&str`), `purist::no_boxed_dyn_error` in library APIs, `purist::exit_code_hygiene` (`std::process::ExitCode` in `main` instead of `process::exit`), and `purist::no_redundant_conversions`.

---

## 3. Review Invariants & Domain Criteria

1. **Zero Unhandled Panics in Production Code:**
   - Replace `.unwrap()` / `.expect()` with `?` propagation, pattern matching, or safe fallbacks (`.unwrap_or_default()`, `let Some(...) = ... else`).
   - Note that `clippy::expect_used` is sometimes unconfigured when `.expect()` is reserved for statically proven invariants; reviewers must still scrutinize every `.expect(...)` in non-test code and reject any triggered by external input, I/O, UTF-8 decoding, regex compilation of dynamic strings, or slice indexing (`[i]` -> `.get(i)`).
2. **Structured Domain Error Enums (`thiserror`):**
   - Library crates define typed `#[derive(Debug, thiserror::Error)]` enums preserving underlying `#[source]` / `#[from]` causes and actionable context (e.g., offending `PathBuf` or threshold values) rather than erasing errors into `String`, `Box<dyn Error>`, or `.map_err(|_| ...)`.
3. **No Silent Error Swallowing:**
   - Reject `let _ = fallible()`, `.ok()` without handling `None`, `.unwrap_or_default()` on I/O/parse failures that mask corruption, or silently returning empty collections on fatal subprocess/parse errors unless graceful fallback is an explicit `SPEC.md` invariant.
4. **Numeric, UTF-8 & Concurrency Safety:**
   - Use `try_from` / checked or saturating arithmetic over lossy `as` casts; slice strings only at char/delimiter boundaries (`split_once`, `strip_prefix`, `char_indices`).

### False-Positive Exclusions

- Documented graceful degradation explicitly mandated by `SPEC.md` (e.g., skipping an optional external formatter when the binary is absent) is intentional resilience, not silent failure.

---

## 4. Subagent Dispatch & Finding Contract

- **Inputs:** Target Rust files / `jj --no-pager diff` and `code-review check` diagnostics.
- **Output Schema:**
  - `verdict`: `"PASS"` | `"FAIL"`
  - `findings`: List of `{ severity: "CRITICAL" | "MAJOR" | "MINOR", file_span: "<path>:<line>", category: "robustness::<subrule>", issue: "<panic, swallowed error, or unsafe cast>", fix: "<fallible propagation or safe API>" }`. Empty when compliant.

---

## 5. Playbook Hygiene & Verification

- `skills/reviewing-rust-robustness/SKILL.md` starts with YAML frontmatter (`name: reviewing-rust-robustness`, `description: Use when...`), stays strictly `< 500` words (`wc -w`), and includes checklist, subagent prompt, rationalization table, and red flags.
- Verified via `prettier --check "**/*.{md,json}"` and `cargo run -p code-review -- check`.
