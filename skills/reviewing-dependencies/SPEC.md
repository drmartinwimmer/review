# Skill Specification: Dependency Hygiene & Security Review (`reviewing-dependencies`)

## 1. Purpose and Scope

The `reviewing-dependencies` skill (`skills/reviewing-dependencies/SKILL.md`) evaluates proposed additions, version updates, feature flags, and transitive footprints in `Cargo.toml` and `Cargo.lock` to keep the workspace lightweight, maintainable, license-clean, and resilient against supply-chain risks.

- **In Scope:** Dependency necessity (YAGNI vs. `std` or workspace crates), crate ecosystem maturity and established alternatives, maintenance health, RustSec vulnerability & unmaintained advisories (`cargo audit`), typosquatting checks, transitive tree bloat (`cargo tree`), build-time execution risks (`build.rs` and procedural macros), `unsafe` exposure, and permissive license compatibility.
- **Out of Scope:** Source-level Rust modularity (`reviewing-rust-modularity`) and public Rust symbol exports (`reviewing-api-surface`), except where third-party types leak into public signatures.

---

## 2. Mechanical Tool Synergy

Before qualitative judgment, the reviewer inspects:

1. `cargo audit` (and `code-review check`): Flags RustSec CVEs (`audit::advisory`), yanked versions, and unmaintained/unsound warnings (`audit::warning`).
2. `CARGO_BUILD_WARNINGS=deny cargo clippy --all-targets --all-features -- -D warnings`: Enforces `[workspace.lints.cargo]` rules including `cargo::unused_dependencies` and `cargo::unused_workspace_dependencies`.
3. `cargo info <crate>` & `cargo tree -p <crate>` / `cargo tree -i <crate>`: Inspects crate metadata (license, repository, recent version, features) and transitive dependency depth/breadth.

---

## 3. Review Invariants & Domain Criteria

1. **Necessity & YAGNI (Stdlib First):**
   - Reject dependencies added for trivial tasks cleanly solvable in ~10–30 lines of Rust `std` (e.g., basic string/path helpers, simple env parsing, trivial retry loops, or pulling in `regex`/`tokio` for static prefix matching or synchronous CLI execution).
   - Reject crates that duplicate capabilities already present in `[workspace.dependencies]` (e.g., adding `anyhow` or a second TOML/JSON parser when `thiserror`, `toml_edit`, and `serde_json` are already standardized).
   - Require `default-features = false` (or minimal feature selection) when default features pull in unneeded codecs, TLS stacks, or async runtimes.
2. **Popularity, Maintenance & De Facto Standards:**
   - Prefer established, audited ecosystem standards (`clap`, `serde`/`serde_json`, `thiserror`, `toml_edit`, `tempfile`, `googletest`, `syn`/`quote`) over obscure, low-adoption, single-maintainer hobby crates (`obscure_deps`).
   - Flag crates showing abandonment signals (multi-year release staleness with open soundness bugs, archived repositories, or yanked recent versions).
3. **Security, Typosquatting & Build-Time Execution:**
   - Zero tolerance (`CRITICAL`) for unresolved RustSec advisories, yanked versions, or look-alike package names (`serde_josn`, `clapp`, etc.).
   - Scrutinize new `build.rs` scripts, native `-sys` C/C++ bindings, and heavy proc-macro crates that execute arbitrary code or network calls during compilation inside developer/CI sandboxes; prefer pure-Rust implementations.
4. **Permissive License Compatibility:**
   - This repository is MIT-licensed (`LICENSE`). Dependencies must use permissive MIT/Apache-2.0-compatible licenses (`MIT`, `Apache-2.0`, `BSD-*`, `ISC`, `Unicode-3.0`, `Zlib`).
   - Strictly flag (`CRITICAL`) copyleft (`GPL-*`, `AGPL-*`, `LGPL-*`), non-commercial, or unlicensed crates.

### False-Positive Exclusions

- Do not flag `[dev-dependencies]` (`googletest`, `tempfile`) for production binary size, though license, CVE, and `build.rs` checks still apply.
- Do not reject a well-justified domain crate (e.g., `toml_edit` for comment-preserving TOML AST edits) where `std` cannot satisfy spec invariants.

---

## 4. Subagent Dispatch & Finding Contract

- **Inputs:** `jj --no-pager diff` on `Cargo.toml` / `Cargo.lock`, `cargo audit` output, and `cargo tree` delta.
- **Output Schema:**
  - `verdict`: `"PASS"` | `"FAIL"`
  - `findings`: List of `{ severity: "CRITICAL" | "MAJOR" | "MINOR", file_span: "Cargo.toml:<line>", category: "dependencies::<subrule>", issue: "<concise description>", fix: "<std replacement, standard crate, or feature trim>" }`. Empty when compliant.

---

## 5. Playbook Hygiene & Verification

- `skills/reviewing-dependencies/SKILL.md` starts with YAML frontmatter (`name: reviewing-dependencies`, `description: Use when...`), stays strictly `< 500` words (`wc -w`), and includes checklist, subagent prompt, rationalization table, and red flags.
- Verified via `prettier --check "**/*.{md,json}"` and `cargo run -p code-review -- check`.
