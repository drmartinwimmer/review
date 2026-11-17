---
name: reviewing-rust-robustness
description: Use when reviewing production Rust error handling, Option/Result propagation, string or slice indexing, numeric conversions, or unsafe blocks to eliminate panics, swallowed errors, and unstructured String errors.
---

# Reviewing Rust Robustness

Production Rust code must never panic on runtime data or silently discard failures. Enforce typed error propagation, checked indexing/casts, and explicit handling of every fallible operation. Full contracts: [SPEC.md](./SPEC.md).

## Mechanical Pre-Checks

Run before line-by-line review:

1. `CARGO_BUILD_WARNINGS=deny cargo clippy --all-targets --all-features -- -D warnings` — enforces `unwrap_used`, `indexing_slicing`, `string_slice`, `panic`, `todo`, `unimplemented`, `unreachable`, `let_underscore_must_use`, `let_underscore_future`, `unused_result_ok`, `map_err_ignore`, `undocumented_unsafe_blocks`, and checked numeric cast lints.
2. `cargo run -p code-review -- purist` — enforces `error_types`, `no_boxed_dyn_error`, `exit_code_hygiene`, and `no_redundant_conversions`.

## Review Checklist

1. **Zero Runtime Panics (`unwrap_panics`)**
   - Reject `.unwrap()`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, direct slice indexing `xs[i]`, and byte-offset string slicing `&s[a..b]` in non-test code.
   - Scrutinize every `.expect(...)` in production paths: reject if reachable via malformed input, I/O failure, non-UTF-8 output, or empty collections; replace with `?`, `.get()`, `split_once`, or `strip_prefix`.
2. **Structured Error Enums (`thiserror`)**
   - Libraries define `#[derive(Debug, thiserror::Error)]` enums preserving `#[source]`/`#[from]` chains and diagnostic context—never `Result<T, String>`, `Result<T, &str>`, or `Box<dyn Error>`.
3. **No Silent Error Swallowing**
   - Reject `let _ = fallible()`, `.ok()`, `.map_err(|_| ...)`, or `.unwrap_or_default()` on I/O or parse results that mask real failures unless fallback behavior is explicitly specified in `SPEC.md`.
4. **Numeric, Process & Memory Safety**
   - Replace lossy `as` casts with `try_from` / checked or saturating ops; return `ExitCode` from `main` instead of `std::process::exit`; require `// SAFETY:` proofs on single-op `unsafe` blocks (prefer safe Rust).

## Subagent Prompt & Output Format

```text
Review non-test Rust code in <target_path_or_diff> using skills/reviewing-rust-robustness/SKILL.md.
Run `code-review check`, audit every panic vector, error type, cast, and ignored Result, and return:
Verdict: PASS | FAIL
Findings (empty if clean—no speculative nitpicks):
- [CRITICAL|MAJOR|MINOR] <file>:<line> — robustness::<rule>: <issue> | Fix: <remediation>
```

## Common Rationalizations

| Excuse                                                      | Reality                                                                            |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| "We checked `.len()` above, so `slice[0]`/`.expect` works." | Refactors break distant preconditions; use `.first()` / `let [first, ..] = slice`. |
| "`Result<T, String>` is fine for an internal module."       | Erases error variants and source chains; define a small `thiserror::Error` enum.   |
| "`unwrap_or_default()` keeps the caller infallible."        | Conflates "empty file" with "permission denied / corrupt JSON"—propagate `Result`. |
| "`count as u32` won't overflow in practice."                | Use `u32::try_from(count)` or `.unwrap_or(u32::MAX)` so truncation is impossible.  |

## Red Flags — STOP

- Any `.unwrap()`, `.expect()`, `[idx]`, or `&str[a..b]` on external, CLI, VFS, or subprocess data.
- `let _ = ...`, `.ok()`, or `.unwrap_or_default()` hiding I/O, serialization, or subprocess errors.
- `Result<_, String>`, `Box<dyn Error>` in library signatures, or `process::exit` inside libraries.
