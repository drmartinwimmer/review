# Module Specification: Code Coverage Engine (`code-review-coverage`)

## 1. Purpose and Overview

The `code-review coverage` command automates LLVM source-based code coverage collection, reporting, and threshold enforcement across a Cargo crate or workspace using `cargo-llvm-cov`.

It executes `cargo llvm-cov --all-targets --all-features --json`, parses the resulting LLVM coverage export JSON payload, computes line, function, branch, and region coverage metrics, reports per-file coverage summaries, and enforces minimum coverage gates (`--fail-under <pct>`).

---

## 2. Invariants and Architectural Guarantees

1. **Centralized Process Execution**: In compliance with `purist::centralized_command_execution`, all subprocess invocations (`std::process::Command::new("cargo")`) are isolated inside `crates/coverage/src/tools/llvm_cov.rs`.
2. **Deterministic Exit Codes**:
   - Exit code `0`: Coverage execution succeeded and line coverage met or exceeded the configured `--fail-under` threshold (if any).
   - Exit code `1`: Measured line coverage fell below the configured `--fail-under` threshold (`CoverageError::ThresholdNotMet`).
   - Exit code `2`: Operational failure (missing target path, `cargo-llvm-cov` failure, I/O error, or malformed JSON).
3. **No Unbuffered Library Output**: Library methods write formatted reports via `std::io::Write` streams (`render_console`, `render_json`, `render_lcov`) rather than direct `println!` macros.
4. **Clean Library Facade**: `crates/coverage/src/lib.rs` acts strictly as a module declaration and re-export facade (`purist::lib_facade_hygiene`).

---

## 3. Command Line Interface Specification

```
code-review coverage [OPTIONS]

Options:
      --path <PATH>              Path to target workspace or crate directory [default: .]
      --fail-under <PCT>         Minimum line coverage percentage threshold (alias: --threshold)
      --json                     Emit coverage report as structured JSON
      --lcov <FILE>              Write LCOV-formatted coverage summary to file
  -q, --quiet                    Silence non-essential output
  -h, --help                     Print help
```

---

## 4. LLVM Coverage JSON Schema & Parsing (`src/parser.rs`)

`cargo llvm-cov --json` emits an LLVM coverage export object with the following structure:

- `data[0].totals.lines`: `{ count, covered, percent }`
- `data[0].totals.functions`: `{ count, covered, percent }`
- `data[0].totals.branches`: `{ count, covered, percent }` (optional; defaults to `0.0` when branch instrumentation is not active)
- `data[0].totals.regions`: `{ count, covered, percent }`
- `data[0].files[]`: `{ filename, summary: { lines, functions, branches, regions } }`

The parser normalizes these metrics into a `CoverageReport` containing a workspace-wide `CoverageSummary` and sorted `Vec<FileCoverage>` entries.
