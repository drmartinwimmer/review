# Skill Specification: Rust Lint & Suppression Hygiene Review (`reviewing-rust-lint-hygiene`)

## 1. Purpose and Scope

The `reviewing-rust-lint-hygiene` skill (`skills/reviewing-rust-lint-hygiene/SKILL.md`) enforces strict compiler, Cargo manifest, Clippy, and `purist` cleanliness across the workspace and prevents agents from silencing static analysis via unjustified `#[allow]` / `#[expect]` attributes or weakened `Cargo.toml` lint tables.

- **In Scope:** Workspace/crate `[lints.clippy]`, `[lints.rust]`, and `[lints.cargo]` configuration integrity (`code-review configure-lints`), zero warnings under `CARGO_BUILD_WARNINGS=deny` and `-D warnings`, strict justification requirements on `#[expect(..., reason = "...")]`, prohibition of `#[allow(...)]` where `#[expect(...)]` applies, and formatting compliance (`rustfmt`, `taplo`, `prettier`).
- **Out of Scope:** Semantic error-handling design (`reviewing-rust-robustness`) and third-party dependency selection (`reviewing-dependencies`), except `[lints.cargo]` unused-dependency enforcement.

---

## 2. Mechanical Tool Synergy

1. `cargo run -p code-review -- check`: Runs `cargo fmt --check`, `cargo clippy --all-targets --all-features`, `purist`, `api-manifest`, `cargo audit`, `taplo`, and `prettier`.
2. `CARGO_BUILD_WARNINGS=deny cargo clippy --all-targets --all-features -- -D warnings`: Denies both `rustc`/Clippy warnings and Cargo 1.100 `[lints.cargo]` manifest warnings.
3. `cargo run -p code-review -- purist`: Enforces `purist::clippy_suppression_hygiene` (flags `#[allow]` / `#[expect]` missing an explicit `reason = "..."` or preceding explanation comment).
4. `cargo run -p code-review -- configure-lints`: Injects and verifies the canonical 67-lint `Strict` (or 64-lint `Standard`) preset in `Cargo.toml`.

---

## 3. Review Invariants & Domain Criteria

1. **Intact Strict Lint Configuration:**
   - Member crates in a workspace must inherit `[lints] workspace = true` rather than omitting `[lints]` or overriding workspace lints to `"allow"` in `Cargo.toml` or `clippy.toml`.
   - Standalone crates must maintain the `code-review configure-lints` preset (`[lints.clippy]`, `[lints.rust]`, `[lints.cargo]`).
2. **Fix the Code, Never Paper Over Lints (`unjustified_suppression`):**
   - Prefer refactoring the underlying code (splitting a long function, replacing indexing with `.get()`, deleting dead code or unused dependencies) over adding a suppression attribute.
   - `#[allow(...)]` is banned (`clippy::allow_attributes = "warn"`): any genuinely unavoidable suppression must use narrow-scope `#[expect(lint_name, reason = "...")]` so the compiler warns as soon as the suppression becomes stale (`unfulfilled_lint_expectations`).
3. **Substantive Suppression Reasons Only:**
   - Even when `reason = "..."` satisfies syntax checks, reviewers must reject tautological or evasive reasons (e.g., `reason = "clippy warning"`, `reason = "safe"`, `reason = "needed here"`, `reason = "legacy"`). A valid reason must state the concrete invariant or external FFI/API constraint proving safety or necessity.
   - Crate-level `#![allow(...)]` or module-wide suppressions are `CRITICAL` defects.
4. **Zero Formatting or Manifest Warnings:**
   - All Rust, TOML, Markdown, and JSON files must pass `cargo fmt --check`, `taplo fmt --check`, and `prettier --check`.

### False-Positive Exclusions

- Narrow item-level `#[expect(..., reason = "...")]` citing a genuine external constraint (e.g., serde/clap derive signature requirements or documented bit-width invariant) is valid.

---

## 4. Subagent Dispatch & Finding Contract

- **Inputs:** `Cargo.toml`, `clippy.toml`, changed `.rs` files (`jj --no-pager diff`), and `code-review check` output.
- **Output Schema:**
  - `verdict`: `"PASS"` | `"FAIL"`
  - `findings`: List of `{ severity: "CRITICAL" | "MAJOR" | "MINOR", file_span: "<path>:<line>", category: "lint_hygiene::<unjustified_suppression|broad_allow|weakened_manifest|warning>", issue: "<hygiene violation>", fix: "<refactor removing suppression or exact scoped #[expect]>" }`. Empty when compliant.

---

## 5. Playbook Hygiene & Verification

- `skills/reviewing-rust-lint-hygiene/SKILL.md` starts with YAML frontmatter (`name: reviewing-rust-lint-hygiene`, `description: Use when...`), stays strictly `< 500` words (`wc -w`), and includes checklist, subagent prompt, rationalization table, and red flags.
- Verified via `prettier --check "**/*.{md,json}"` and `cargo run -p code-review -- check`.
