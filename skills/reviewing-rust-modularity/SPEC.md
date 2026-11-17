# Skill Specification: Rust Modularity & Architecture Review (`reviewing-rust-modularity`)

## 1. Purpose and Scope

The `reviewing-rust-modularity` skill (`skills/reviewing-rust-modularity/SKILL.md`) evaluates file organization, module boundaries, single-responsibility cohesion, facade cleanliness, and idiomatic Rust structural patterns.

- **In Scope:** Separate submodule files vs. inline `mod` blocks, `lib.rs` / `main.rs` facade hygiene, free functions vs. stateless unit-struct namespaces, file/function size & nesting complexity, `use` import hygiene, and separation of pure domain/parsing logic from I/O and subprocess execution.
- **Out of Scope:** External `Cargo.toml` crate vetting (`reviewing-dependencies`), panic/error-value semantics (`reviewing-rust-robustness`), and `API.md` SemVer lifecycle (`reviewing-api-surface`).

---

## 2. Mechanical Tool Synergy

1. `cargo run -p code-review -- purist`: Mechanically enforces:
   - `purist::no_inline_mods` (forbids non-test inline `mod foo { ... }` in `lib.rs` / `main.rs`)
   - `purist::lib_facade_hygiene` (keeps `lib.rs` as a clean module declaration and re-export facade)
   - `purist::free_functions` (flags stateless dummy structs used purely as method namespaces)
   - `purist::max_file_lines` and `purist::max_nesting_depth`
   - `purist::no_wildcard_imports` and `purist::use_declarations_over_qualified_paths`
   - `purist::no_println_in_libraries`, `purist::no_primitive_flag_clutter`, `purist::no_redundant_wrappers`, and `purist::single_match_to_let_else`
2. `cargo clippy --all-targets --all-features -- -D warnings`: Enforces `clippy::cognitive_complexity`, `clippy::too_many_lines`, and `clippy::manual_let_else`.

---

## 3. Review Invariants & Domain Criteria

1. **Dedicated Submodule Files & Thin Entrypoints:**
   - Every non-test submodule lives in its own file (`src/foo.rs` or `src/foo/mod.rs`).
   - `main.rs` handles CLI parsing and exit-code mapping only; `lib.rs` declares modules and re-exports the crate's public facade without embedding heavy implementation logic.
2. **Single Responsibility & Pure-Core / Imperative-Shell Separation:**
   - Subprocess execution (`tools/*.rs`), filesystem I/O, parser logic (`parser.rs`), and CLI command orchestration (`command.rs`) must be separated into focused modules so parsers and evaluators are unit-testable without spawning processes.
   - Library functions write formatted output to injected `impl std::io::Write` streams rather than calling `println!` / `eprintln!` directly.
3. **Idiomatic Free Functions Over Java-Style Namespaces:**
   - Stateless operations belong as module-level free functions (`parser::parse_output(...)`), never attached to zero-field unit structs (`struct OutputParser; impl OutputParser { ... }`) unless implementing a trait.
4. **Shallow Control Flow & Narrow Signatures:**
   - Prefer `let ... else { return ... };` guard clauses and focused helpers over deeply nested `match`/`if let` pyramids or boolean parameter clutter (`bool, bool, bool`).

### False-Positive Exclusions

- Inline `#[cfg(test)] mod tests { ... }` at the bottom of an implementation module is idiomatic Rust and explicitly permitted.
- Unit structs that implement a required trait (e.g., `syn::visit::Visit` or a `Rule` trait) are legitimate polymorphism, not dummy namespaces.

---

## 4. Subagent Dispatch & Finding Contract

- **Inputs:** Target Rust files / `jj --no-pager diff` and `code-review purist` diagnostics.
- **Output Schema:**
  - `verdict`: `"PASS"` | `"FAIL"`
  - `findings`: List of `{ severity: "CRITICAL" | "MAJOR" | "MINOR", file_span: "<path>:<line>", category: "modularity::<subrule>", issue: "<structural smell>", fix: "<module split, free function, or facade cleanup>" }`. Empty when compliant.

---

## 5. Playbook Hygiene & Verification

- `skills/reviewing-rust-modularity/SKILL.md` starts with YAML frontmatter (`name: reviewing-rust-modularity`, `description: Use when...`), stays strictly `< 500` words (`wc -w`), and includes checklist, subagent prompt, rationalization table, and red flags.
- Verified via `prettier --check "**/*.{md,json}"` and `cargo run -p code-review -- check`.
