# code-review

> **Unified Rust CLI toolkit for automated code review, static analysis, API drift auditing, lint configuration, and coverage gating.**

`code-review` aggregates static analysis, formatting, AST lint rules, public API manifest verification, Clippy configuration, and LLVM code coverage into a single CLI designed for both human developers and coding agents.

## Subcommands

- **`code-review check`**: Runs `cargo fmt`, `cargo clippy`, `purist`, `api-manifest`, `cargo audit`, and Markdown/TOML/JSON formatters with unified diagnostics and Jujutsu `--changed-only` filtering.
- **`code-review configure-lints`**: Injects or updates strict Clippy and Rust lints in `Cargo.toml` while preserving comments and formatting.
- **`code-review purist`**: Executes AST-based architectural, hygiene, and test-quality rules powered by `purist`.
- **`code-review api`**: Generates and verifies checked-in `API.md` surface manifests across libraries, CLIs, HTTP services, and config files via `api-manifest`.
- **`code-review coverage`**: Runs `cargo-llvm-cov` and enforces `--fail-under` coverage thresholds.

## Installation

```bash
cargo install code-review
```

## Usage

```bash
# Run all quality, formatting, security, and API checks
code-review check

# Scope checks to files modified in the active Jujutsu change
code-review check --changed-only

# Configure strict Clippy lints in Cargo.toml
code-review configure-lints

# Verify checked-in API.md manifests
code-review api check

# Enforce coverage threshold
code-review coverage --fail-under 80
```
