---
name: reviewing-rust-lint-hygiene
description: Use when reviewing Rust compiler warnings, Cargo.toml or clippy.toml lint configurations, formatting compliance, or #[allow] and #[expect] suppression attributes to prevent lint evasion and warning debt.
---

# Reviewing Rust Lint Hygiene

Zero warnings, intact strict lint presets, and zero lazy suppressions. Fix the underlying code rather than silencing the linter. Full contracts: [SPEC.md](./SPEC.md).

## Mechanical Pre-Checks

Run before inspecting suppressions:

1. `cargo run -p code-review -- check` — runs `rustfmt`, `clippy`, `purist`, `api-manifest`, `cargo audit`, `taplo`, and `prettier`.
2. `CARGO_BUILD_WARNINGS=deny cargo clippy --all-targets --all-features -- -D warnings` — denies compiler, Clippy, and Cargo 1.100 `[lints.cargo]` manifest warnings.
3. `cargo run -p code-review -- purist` — enforces `purist::clippy_suppression_hygiene`.

## Review Checklist

1. **Intact Strict Lint Configuration**
   - Workspace members inherit `[lints] workspace = true`; root/standalone `Cargo.toml` preserves the `code-review configure-lints` preset (`[lints.clippy]`, `[lints.rust]`, `[lints.cargo]`) without downgrading rules to `"allow"` in `Cargo.toml` or `clippy.toml`.
2. **Refactor Over Suppressing (`unjustified_suppression`)**
   - Reject suppressions added to dodge refactoring (e.g., suppressing `too_many_lines`, `cognitive_complexity`, `unwrap_used`, `indexing_slicing`, or `unused_dependencies`).
   - `#[allow(...)]` is forbidden (`clippy::allow_attributes`); any truly unavoidable exception must use narrow item/expression-scoped `#[expect(lint, reason = "...")]` so stale suppressions fail compilation.
3. **Substantive Justification Audit**
   - Reject vague or circular `reason` strings (`"safe"`, `"clippy"`, `"needed"`, `"ok here"`, `"false positive"` without proof). Require the concrete invariant, FFI contract, or upstream macro limitation.
   - Reject (`CRITICAL`) crate-wide `#![allow(...)]` / `#![expect(...)]` or module-level blanket suppressions.
4. **Workspace-Wide Formatter Cleanliness**
   - All `.rs`, `.toml`, `.md`, and `.json` files pass `cargo fmt --check`, `taplo fmt --check`, and `prettier --check`.

## Subagent Prompt & Output Format

```text
Review lint configuration, formatting, and #[allow]/#[expect] attributes in <target_path_or_diff> using skills/reviewing-rust-lint-hygiene/SKILL.md.
Run `code-review check` and `CARGO_BUILD_WARNINGS=deny cargo clippy --all-targets --all-features -- -D warnings`, and return:
Verdict: PASS | FAIL
Findings (empty if clean—no speculative nitpicks):
- [CRITICAL|MAJOR|MINOR] <file>:<line> — lint_hygiene::<rule>: <issue> | Fix: <remediation>
```

## Common Rationalizations

| Excuse                                                        | Reality                                                                                  |
| ------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| "`#[expect(clippy::too_many_lines, reason = \"cli\")]` fits." | Tautological reason hiding a bloated function; extract focused helpers instead.          |
| "`#[allow(...)]` works on both triggered and clean builds."   | That is the bug: `#[allow]` rots silently once fixed; `#[expect]` self-polices staleness |
| "I'll relax the lint in `Cargo.toml` for this crate."         | Weakens safety for the entire crate; keep `[lints] workspace = true`.                    |
| "Cargo manifest warnings aren't real compiler warnings."      | Unused deps and malformed metadata bloat releases; `CARGO_BUILD_WARNINGS=deny` blocks it |

## Red Flags — STOP

- Any `#[allow(...)]` attribute or crate/module-level `#![expect(...)]`.
- `#[expect(..., reason = "...")]` with a restatement of the lint name instead of an invariant.
- Removed or weakened entries in `[workspace.lints.*]` or missing `[lints] workspace = true`.
