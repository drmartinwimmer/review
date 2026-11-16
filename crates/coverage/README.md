# code-review-coverage

> **LLVM source-based code coverage driver and threshold gate for Rust workspaces.**

`code-review-coverage` wraps `cargo-llvm-cov` to collect line, function, branch, and region coverage metrics across Cargo crates and workspaces, render human- and machine-readable summaries, export LCOV reports, and enforce `--fail-under` coverage thresholds.

## Features

1. **Multi-dimension coverage metrics**: Line, function, branch, and region summaries
2. **Threshold gating**: `--fail-under <pct>` exits with code `1` when line coverage drops below target
3. **Flexible output formats**: Console summary tables, structured JSON (`--json`), and LCOV files (`--lcov <path>`)

## Installation

```bash
cargo install code-review-coverage --bin coverage
```

## Usage

```bash
# Measure coverage across current crate or workspace
coverage

# Enforce an 80% minimum line coverage threshold
coverage --fail-under 80

# Emit JSON report and write LCOV output
coverage --json --lcov lcov.info
```
