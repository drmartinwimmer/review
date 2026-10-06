# Module Specification: API Manifest Engine & Auditor (`code-review-api`)

## 1. Purpose and Overview

The `code-review api` tool provides automated introspection, manifest generation, and drift auditing for public interfaces. By generating a canonical, human-readable API manifest (`API.md` or JSON), teams ensure that every API addition, modification, or removal is intentional, reviewed, and versioned in accordance with Semantic Versioning (SemVer).

The engine supports three distinct public interface surfaces:

1. **Library Targets (`library.rs`)**: Introspects public items (`pub struct`, `pub enum`, `pub fn`, `pub trait`, `pub type`, `pub const`, and public re-exports) exported by Rust library crates.
2. **Binary / CLI Targets (`cli.rs`)**: Introspects command-line interfaces (subcommands, arguments, flags, options, defaults, and documentation).
3. **HTTP Web Service Targets (`http.rs`)**: Detects route registrations (e.g., Axum router patterns or attribute routes), extracting paths, HTTP methods, and payload types.

The tool provides two primary operations:

- `code-review api dump`: Introspects target crates and serializes their public interface into `API.md` (or JSON).
- `code-review api check`: Compares the active codebase against the checked-in `API.md`, categorizing drift into additions, modifications, and breaking removals, and reporting findings as structured diagnostics.

---

## 2. Invariants and Architectural Guarantees

1. **Deterministic Manifest Output**:
   - Extracted API items must be strictly ordered (sorted lexicographically by kind, path, and identifier) to prevent spurious diffs across environments or compilation runs.
2. **Strict Public Visibility Filtering**:
   - Only genuinely public items (`pub`, but _not_ `pub(crate)`, `pub(super)`, `pub(in ...)`, or private items) are captured in library manifests.
3. **SemVer Drift Classification**:
   - Removals of public items, CLI arguments, or HTTP endpoints are classified as **breaking** changes (`Severity::Error`).
   - Additions of public items, optional CLI flags, or new HTTP endpoints are classified as **non-breaking** additions (`Severity::Warning` or `Severity::Info`).
   - Signature changes or newly required CLI arguments without defaults are classified as **breaking** modifications (`Severity::Error`).
4. **AST-Driven Static Extraction**:
   - Extractors analyze Rust source code statically using `syn` to avoid executing target binaries or requiring target linking.
5. **Centralized Process & I/O Isolation**:
   - In compliance with `purist::no_println_in_libraries`, library functions must not emit unbuffered `println!` or `eprintln!` directly; results are returned via `Result<ApiReport, ApiError>` or formatted into `code_review_diagnostics::DiagnosticReport`.
6. **Deterministic Exit Codes**:
   - Exit code `0`: Manifest check succeeded with zero drift (or additions accepted).
   - Exit code `1`: API drift or breaking changes detected against checked-in `API.md`.
   - Exit code `2`: Operational error (e.g., path not found, manifest missing, invalid argument, syntax failure).

---

## 3. Command Line Interface Specification

```
code-review api [OPTIONS] [COMMAND]

Commands:
  dump   Inspect codebase and generate or update the API manifest (API.md)
  check  Compare codebase against the checked-in API manifest and detect drift

Options:
      --path <PATH>              Path to target crate or workspace [default: .]
      --manifest <FILE>          Path to API manifest file [default: API.md]
      --format <FORMAT>          Report output format: console, json, markdown [default: console]
      --fail-on <LEVEL>          Drift threshold triggering exit code 1: any, breaking [default: any]
  -q, --quiet                    Silence non-essential status messages
  -h, --help                     Print help
```

### 3.1 `dump` Subcommand

```
code-review api dump [OPTIONS]

Options:
      --path <PATH>              Path to target crate or workspace [default: .]
  -o, --output <FILE>            Output manifest file path [default: API.md]
      --format <FORMAT>          Manifest format: markdown, json [default: markdown]
  -q, --quiet                    Silence status messages
```

### 3.2 `check` Subcommand

```
code-review api check [OPTIONS]

Options:
      --path <PATH>              Path to target crate or workspace [default: .]
      --manifest <FILE>          Path to checked-in manifest [default: API.md]
      --fail-on <LEVEL>          Failure threshold: any, breaking [default: any]
      --format <FORMAT>          Output format: console, json, markdown [default: console]
  -q, --quiet                    Silence status messages
```

---

## 4. Target Surface Extractors

### 4.1 Library Extractor (`extractors/library.rs`)

- Traverses `src/lib.rs` and nested public modules declared via `pub mod`.
- Extracts:
  - **Structs**: name, generics, public fields (name and type), public methods from inherent `impl` blocks.
  - **Enums**: name, generics, variants, and payload signatures.
  - **Functions**: name, generics, parameter types, return types, qualifiers (`const`, `async`, `unsafe`).
  - **Traits**: name, generics, supertraits, associated types, method signatures.
  - **Type Aliases**: name, generics, aliased target type.
  - **Constants**: name and type.
  - **Re-exports**: public `pub use` items pointing to internal or external items.

### 4.2 CLI Extractor (`extractors/cli.rs`)

- Statically inspects CLI declarations using `syn` AST analysis and Clap attribute introspection:
  - Detects `#[derive(Parser)]`, `#[derive(Args)]`, `#[derive(Subcommand)]`, and `#[command(...)]` / `#[arg(...)]` attributes.
  - Also supports dynamic introspection from a `clap::Command` builder instance when available.
- Extracts:
  - Command name, version, and about text.
  - Subcommands and nested subcommand trees.
  - Flags and options: short flag, long flag, value name, required vs optional, default value, environment variable bindings, and doc summaries.
  - Positional arguments: index, name, required vs optional, help text.

### 4.3 HTTP Service Extractor (`extractors/http.rs`)

- Statically detects route definitions from popular Rust web frameworks (e.g., Axum router method chains and route attribute macros):
  - Detects `.route("path", get(handler))` and `.route("path", post(handler))` patterns.
  - Detects attribute macros such as `#[get("path")]`, `#[post("path")]`, and `#[utoipa::path(...)]`.
- Extracts:
  - Route path template (e.g. `/api/v1/health`, `/users/{id}`).
  - HTTP method (`GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`).
  - Handler function identifier.

---

## 5. API Manifest Specification (`API.md`)

The generated `API.md` is formatted as clean, human-readable GitHub Flavored Markdown:

1. **Header**: `# API Manifest: <crate_name> (v<version>)`
2. **Library API**:
   - Subsections for Structs, Enums, Traits, Functions, Type Aliases, and Constants.
   - Normalized Rust signatures in code fences.
3. **CLI API**:
   - Subcommands and arguments formatted in clean markdown tables.
   - Columns: Flag/Argument, Type, Required, Default, Description.
4. **HTTP API**:
   - Route endpoint tables.
   - Columns: Method, Path, Handler, Description.

---

## 6. Drift Detection & Breaking Change Rules

The differ (`diff.rs`) compares the current active surface against the manifest:

1. **Added Items**:
   - An item present in active surface but absent in manifest.
   - Marked as `+ [ADD]` with `Severity::Warning`.
2. **Removed Items**:
   - An item present in manifest but absent in active surface.
   - Marked as `- [REMOVE (BREAKING)]` with `Severity::Error`.
3. **Modified Items**:
   - Signatures differ:
     - Function parameters changed or return type modified: `~ [MODIFY (BREAKING)]` (`Severity::Error`).
     - CLI argument made required without default: `~ [MODIFY (BREAKING)]` (`Severity::Error`).
     - CLI argument added as optional or with default: `~ [MODIFY (ADDITION)]` (`Severity::Warning`).
     - HTTP endpoint path or method changed: `~ [MODIFY (BREAKING)]` (`Severity::Error`).

---

## 7. Diagnostics & Aggregator Integration

When run via `code-review check`, the drift auditor produces standard `Diagnostic` records:

- `rule`: `api::drift::removal`, `api::drift::addition`, `api::drift::modification`, `api::manifest_missing`.
- `severity`: `Error` for breaking changes, `Warning` for non-breaking additions/drift.
- `message`: Clear explanation of the drifted item and suggested action (e.g. `Run 'code-review api dump' to update the API manifest`).

---

## 8. Error Taxonomy & Exit Codes

```rust
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Target path '{0}' was not found")]
    PathNotFound(PathBuf),

    #[error("API manifest not found at '{0}'")]
    ManifestNotFound(PathBuf),

    #[error("Failed to parse Rust source at '{path}': {message}")]
    SourceParseError { path: PathBuf, message: String },

    #[error("I/O error during API inspection: {0}")]
    Io(#[from] std::io::Error),

    #[error("API manifest drift detected ({count} issues exceed threshold)")]
    DriftDetected { count: usize, breaking_count: usize },
}
```

Exit Codes:

- `0`: Pass (no drift or acceptable additions).
- `1`: Violations exceed threshold (`--fail-on`).
- `2`: Operational error (I/O, invalid arguments, parsing failure).

---

## 9. Testing Strategy

1. **Unit Tests (`extractors/library.rs`)**: Test public struct, enum, trait, function, and type alias extraction with positive and negative visibility cases.
2. **Unit Tests (`extractors/cli.rs`)**: Test clap command introspection and AST parser with subcommands, flags, options, and defaults.
3. **Unit Tests (`extractors/http.rs`)**: Test Axum router method chains and route attribute parsing.
4. **Unit Tests (`manifest.rs`)**: Test serialization to clean markdown and JSON.
5. **Unit Tests (`diff.rs`)**: Test diffing identical surfaces, added items, removed items, and modified signatures.
6. **Integration Tests (`crates/api/tests/cli.rs`)**: End-to-end testing of `code-review api dump` and `code-review api check`.
