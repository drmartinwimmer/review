# Module Specification: Check Aggregator (`code-review-check`)

## 1. Purpose and Overview

The `code-review check` command is the central quality and conformance aggregator of the `code-review` toolkit. It coordinates the execution of static analysis tools, linters, formatters, and security scanners across a Cargo project or workspace, synthesizing their output into a unified `miette::Diagnostic`-based model (`DiagnosticReport` containing `CheckDiagnostic` items).

The aggregator executes:

1. **Formatting Check (`cargo fmt --check`)**: Verifies that all Rust source code conforms to the standard rustfmt configuration.
2. **Clippy & Compiler Lints (`cargo clippy --message-format=json`)**: Runs compiler and Clippy static analysis with warnings/errors streamed as structured JSON.
3. **Purist AST Rules (`code-review purist`)**: Runs custom AST-based architecture, style, and hygiene rules via `purist`.
4. **Security Vulnerability Audit (`cargo audit --json`)**: Scans dependencies in `Cargo.lock` against the RustSec Advisory Database.
5. **Markdown Format & Lint Check (`prettier --check` / `mdformat --check`)**: If available in the environment, verifies formatting and syntax of `*.md` files.
6. **TOML Format & Lint Check (`taplo fmt --check` / in-process `toml_edit`)**: If available in the environment, verifies formatting and syntax of `*.toml` files.
7. **JSON Format & Lint Check (`prettier --check` / in-process `serde_json`)**: If available in the environment, verifies formatting and syntax of `*.json` files.
8. **API Manifest Drift Check (`code-review api check`)**: Verifies checked-in `API.md` manifests against the active codebase.
9. **Jujutsu Changed-File Filtering (`--changed-only`)**: When enabled, queries `jj --no-pager diff --summary` to scope reported diagnostics exclusively to files modified or added in the active working revision.

---

## 2. Invariants and Architectural Guarantees

1. **Typed `miette::Diagnostic` Hierarchy**: Each underlying checker produces concrete `<Tool>Diagnostic` types implementing `thiserror::Error` and `miette::Diagnostic`, flattened transparently into `CheckDiagnostic` with standardized diagnostic codes, `miette::Severity` (`Error`, `Warning`, `Advice`), optional source code spans (`purist::Span` implementing `miette::SourceCode` and `Into<miette::SourceSpan>`), messages, and actionable `help()` suggestions.
2. **Centralized Process Execution**: In compliance with `purist::centralized_command_execution`, all external process invocations (`std::process::Command::new`) must be isolated within dedicated tool runner modules under `crates/check/src/tools/`.
3. **Resilience & Graceful Degradation ("If Available")**:
   - Optional external checkers/formatters (`prettier`, `mdformat`, `taplo`, `cargo-audit`) probe availability before running.
   - If an optional tool is missing from the environment, it is skipped cleanly or backed by in-process syntax verification without failing the suite.
   - If `cargo-audit` fails to acquire the network lock, it automatically retries with `--no-fetch`.
4. **Deterministic Exit Codes**:
   - Exit code `0`: Check execution succeeded and no violations exceeded the configured `--fail-on` threshold.
   - Exit code `1`: Quality, style, or security violations were detected that meet or exceed the `--fail-on` threshold.
   - Exit code `2`: Operational error (e.g., target directory not found, invalid command-line arguments, or VCS failure when `--changed-only` is requested).
5. **Format Transparency**: Supports all standard diagnostic output formats (`console`, `json`, `markdown`) via `render_report`.
6. **No Library Output Side Effects**: In compliance with `purist::no_println_in_libraries`, library functions must not emit unbuffered `println!` or `eprintln!` directly; execution produces a `DiagnosticReport` or writes to an injected `&mut dyn std::io::Write`.

---

## 3. Command Line Interface Specification

```
code-review check [OPTIONS]

Options:
      --path <PATH>              Path to target workspace or crate directory [default: .]
      --format <FORMAT>          Output format: console, json, markdown [default: console]
      --fail-on <LEVEL>          Severity threshold triggering non-zero exit: warnings, errors [default: warnings]
      --changed-only             Filter diagnostics to only files modified in Jujutsu working copy
      --fmt [<BOOL>]             Run cargo fmt checks [default: true]
      --clippy [<BOOL>]          Run cargo clippy checks [default: true]
      --purist [<BOOL>]          Run purist AST linter checks [default: true]
      --audit [<BOOL>]           Run cargo audit dependency security scan [default: true]
      --markdown [<BOOL>]        Run markdown format/lint checks [default: true]
      --toml [<BOOL>]            Run TOML format/lint checks [default: true]
      --json [<BOOL>]            Run JSON format/lint checks [default: true]
      --api [<BOOL>]             Run API manifest drift checks [default: true]
  -q, --quiet                    Silence non-essential logging output
  -h, --help                     Print help
```

---

## 4. Subprocess Runners & Parsers

### 4.1 Rustfmt Runner (`tools/cargo_fmt.rs`)

- **Execution**: `cargo fmt --all --check [--manifest-path <path>]`
- **Diagnostics (`FmtDiagnostic`)**:
  - Exit code `0`: No formatting violations.
  - Exit code `1`: Parses stderr/stdout for `Diff in <file>:<line>:` (Rust 1.100+) and `Diff in <file> at line <line>:` patterns into `FmtDiagnostic::FormattingDiff` (`code(fmt::formatting)`, `Severity::Warning`, help `"Run 'cargo fmt' to format '<file>'"`).

### 4.2 Cargo Clippy Runner (`tools/cargo_clippy.rs`)

- **Execution**: `cargo clippy --all-targets --all-features --message-format=json`
- **Diagnostics (`ClippyDiagnostic`)**:
  - Parses Cargo manifest lint diagnostics (`cargo::*`, such as `cargo::unused_dependencies` and `cargo::non_kebab_case_packages`) emitted on `stderr`, extracting rule code, severity, `Cargo.toml` span, and help suggestions.
  - Reads line-delimited JSON objects on `stdout` where `reason == "compiler-message"`.
  - Extracts `code.code` as the diagnostic code (e.g., `clippy::unwrap_used`, `E0425`).
  - Maps `level` (`"error"` -> `Severity::Error`, `"warning"` -> `Severity::Warning`, other -> `Severity::Advice`).
  - Extracts the primary span (`is_primary == true`) and suggested fixes from primary or child help spans.

### 4.3 Cargo Audit Runner (`tools/cargo_audit.rs`)

- **Execution**: `cargo audit --json` (with automatic fallback to `cargo audit --no-fetch --json`)
- **Diagnostics (`AuditDiagnostic`)**:
  - Maps `vulnerabilities.list` entries to `AuditDiagnostic::Vulnerability` (`code(audit::vulnerability)`, `Severity::Error`, advisory URL, and upgrade help).
  - Maps `warnings` entries to `AuditDiagnostic::AdvisoryWarning` (`audit::<kind>`, `Severity::Warning`).
  - Emits `AuditDiagnostic::Skipped` (`code(audit::skipped)`, `Severity::Advice`) when `Cargo.lock` or `cargo-audit` is absent.

### 4.4 Purist AST Runner (`tools/purist.rs`)

- **Execution**: Invokes `purist::PuristEngine::check_path` directly in-process.
- **Diagnostics (`PuristDiagnostic`)**: Directly aggregates all `miette::Diagnostic` findings emitted by `purist`.

### 4.5 Markdown Runner (`tools/markdown.rs`)

- **Execution**: Probes for `prettier --check` (or `mdformat --check`). If unavailable, skips cleanly.
- **Diagnostics (`MarkdownDiagnostic`)**:
  - Formatting warnings mapped to `code(fmt::markdown)`, `Severity::Warning`, with tool-specific write help.
  - Syntax errors mapped to `code(fmt::markdown)`, `Severity::Error`.

### 4.6 TOML Runner (`tools/toml.rs`)

- **Execution**: Probes for `taplo fmt --check`. If unavailable, executes in-process syntax validation via `toml_edit`.
- **Diagnostics (`TomlDiagnostic`)**:
  - Formatting violations mapped to `code(fmt::toml)`, `Severity::Warning`.
  - Syntax errors mapped to `code(toml::syntax)`, `Severity::Error`.

### 4.7 JSON Runner (`tools/json.rs`)

- **Execution**: Probes for `prettier --check`. If unavailable, executes in-process syntax validation via `serde_json`.
- **Diagnostics (`JsonDiagnostic`)**:
  - Formatting violations mapped to `code(fmt::json)`, `Severity::Warning`.
  - Syntax errors mapped to `code(json::syntax)`, `Severity::Error`.

### 4.8 API Manifest Drift Runner (`tools/api.rs`)

- **Execution**: Verifies checked-in `API.md` files across the target workspace or crate via `api_manifest::ApiManifest::check_path`.
- **Diagnostics (`ApiDiagnostic`)**:
  - Breaking removals or modifications mapped to `Severity::Error`.
  - Non-breaking additions mapped to `Severity::Warning`.

### 4.9 Jujutsu VCS Filter (`tools/vcs_jj.rs`)

- **Execution**: `jj --no-pager diff --summary`
- **Parsing**:
  - Recognizes `M`, `A`, `C <old> -> <new>`, and `R <old> -> <new>` summary entries.
  - Filters `DiagnosticReport` to changed files while preserving span-less workspace diagnostics.

---

## 5. Error Taxonomy

```rust
#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("Target path '{0}' was not found")]
    PathNotFound(std::path::PathBuf),

    #[error("Jujutsu VCS error: {0}")]
    Vcs(#[from] JjError),

    #[error("API manifest check error: {0}")]
    Api(#[from] ApiError),

    #[error("I/O error during check execution: {0}")]
    Io(#[from] std::io::Error),

    #[error("Check violations found ({count} issues exceed failure threshold)")]
    ViolationsFound { count: usize },
}
```

---

## 6. Testing Strategy

1. **Unit Tests**:
   - `cargo_fmt` parser tests with mock diff output.
   - `cargo_clippy` JSON stream parser tests with mock compiler messages (errors, warnings, spans, suggestions).
   - `cargo_audit` JSON parser tests with mock advisory and warning payloads.
   - `purist` runner tests verifying clean code and lint violation detection.
   - `markdown`, `toml`, and `json` runner tests verifying parser behavior and in-process fallback.
   - `vcs_jj` parser and changed-file filter tests.
   - `diagnostics` and `reporter` tests verifying `miette::Diagnostic` metadata delegation and console/JSON/Markdown rendering.
2. **Integration Tests**:
   - Running `CheckCommand` on a clean mock crate verifying exit code 0.
   - Running `CheckCommand` on a mock crate containing violations verifying exit code 1 and `--fail-on` thresholds.
   - CLI parsing tests verifying all flags and default options.
