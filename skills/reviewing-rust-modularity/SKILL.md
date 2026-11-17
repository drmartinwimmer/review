---
name: reviewing-rust-modularity
description: Use when reviewing Rust module structure, lib.rs or main.rs entrypoints, struct/function organization, or file splits to enforce separate submodule files, single responsibility, thin facades, and free functions over dummy namespaces.
---

# Reviewing Rust Modularity

Keep Rust crates cohesive and navigable: separate submodule files, thin `lib.rs`/`main.rs` entrypoints, pure domain parsers decoupled from I/O, and free functions over Java-style unit-struct namespaces. Full contracts: [SPEC.md](./SPEC.md).

## Mechanical Pre-Checks

Run before structural review:

1. `cargo run -p code-review -- purist` — enforces `no_inline_mods`, `lib_facade_hygiene`, `free_functions`, `max_file_lines`, `max_nesting_depth`, `no_wildcard_imports`, `use_declarations_over_qualified_paths`, `no_println_in_libraries`, `no_primitive_flag_clutter`, `no_redundant_wrappers`, and `single_match_to_let_else`.
2. `CARGO_BUILD_WARNINGS=deny cargo clippy --all-targets --all-features -- -D warnings` — enforces `cognitive_complexity`, `too_many_lines`, and `manual_let_else`.

## Review Checklist

1. **Separate Submodule Files & Thin Facades (`inline_mods`)**
   - Every non-test submodule lives in `src/foo.rs` or `src/foo/mod.rs`—never inline `mod foo { ... }` inside `lib.rs` or `main.rs` (inline `#[cfg(test)] mod tests` is permitted).
   - `main.rs` only parses CLI arguments and maps `ExitCode`; `lib.rs` acts strictly as a module declaration and re-export facade.
2. **Single Responsibility & Pure-Core / Imperative-Shell Split**
   - Separate subprocess/filesystem runners (`src/tools/*.rs`), pure output parsers (`parser.rs`), and CLI orchestration (`command.rs`) so logic is unit-testable without spawning processes.
   - Library functions write to injected `impl std::io::Write` streams rather than calling `println!`/`eprintln!`.
3. **Free Functions Over Dummy Unit Structs**
   - Stateless helpers belong as module free functions (`parse_report(...)`), never methods on zero-field namespace structs (`struct Parser; impl Parser`) unless implementing a trait.
4. **Flat Control Flow & Focused Signatures**
   - Use `let ... else` early returns over deep nesting, explicit `use` imports over repeated qualified paths or wildcards, and typed enums/configs over multi-`bool` parameter lists.

## Subagent Prompt & Output Format

```text
Review <target_path_or_diff> using skills/reviewing-rust-modularity/SKILL.md.
Run `code-review purist` and inspect module boundaries, lib.rs/main.rs facades, and responsibility splits, returning:
Verdict: PASS | FAIL
Findings (empty if clean—no speculative nitpicks):
- [CRITICAL|MAJOR|MINOR] <file>:<line> — modularity::<rule>: <issue> | Fix: <remediation>
```

## Common Rationalizations

| Excuse                                                    | Reality                                                                           |
| --------------------------------------------------------- | --------------------------------------------------------------------------------- |
| "It's a small helper module, inline `mod` in `lib.rs`."   | Inline production modules bloat facades; extract `src/<name>.rs`.                 |
| "`struct Helper;` groups related static functions."       | Rust modules already provide namespaces; export free functions instead.           |
| "Parsing inside the `Command::new` runner is simpler."    | Couples parsing to subprocess execution and blocks fast deterministic unit tests. |
| "`println!` inside the library function avoids plumbing." | Breaks `--quiet`/`--json` callers and testing; accept `&mut impl std::io::Write`. |

## Red Flags — STOP

- Non-test `mod foo { ... }` blocks inside `lib.rs` or `main.rs`.
- Zero-field structs with `impl` blocks implementing no traits.
- Monolithic files mixing Clap definitions, `Command::new`, JSON parsing, and terminal printing.
