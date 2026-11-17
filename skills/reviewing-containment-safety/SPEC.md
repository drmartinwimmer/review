# Skill Specification: Containment, Path & Subprocess Safety Review (`reviewing-containment-safety`)

## 1. Purpose and Scope

The `reviewing-containment-safety` skill (`skills/reviewing-containment-safety/SKILL.md`) audits filesystem path resolution, subprocess execution, environment variable access, bounded execution/pager discipline, and sandbox containment guarantees across Rust tools, scripts, CI workflows, and evaluation harnesses.

- **In Scope:** Workspace/manifest-anchored path resolution vs. fragile CWD-relative paths (`cwd_path_resolution`), centralized subprocess wrappers (`Command::new`), non-interactive CLI flags (`--no-pager`, bounded timeouts, no TTY prompts), path-traversal containment on untrusted/configured paths, centralized environment variable reading, and strict adherence to `fence` / CI sandbox boundaries (read-only host mounts, ephemeral workdirs, network isolation).
- **Out of Scope:** Pure in-memory error type design (`reviewing-rust-robustness`) and `crates.io` supply-chain license audits (`reviewing-dependencies`).

---

## 2. Mechanical Tool Synergy

1. `cargo run -p code-review -- purist`: Enforces:
   - `purist::path_resolution` (flags bare relative `Path::new("...")` / `fs::read("...")` unanchored to an explicit root or `CARGO_MANIFEST_DIR`)
   - `purist::centralized_command_execution` (isolates `std::process::Command::new` inside dedicated `tools/` runner modules)
   - `purist::no_env_access_outside_config` (prevents scattered `std::env::var` reads outside configuration boundaries)
   - `purist::raii_temp_directories` (requires RAII `tempfile::TempDir` over ad-hoc `/tmp` paths)

---

## 3. Review Invariants & Domain Criteria

1. **Root-Anchored Path Resolution (`cwd_path_resolution`):**
   - Never assume the process current working directory (`CWD`) equals the repository or crate root; tools may be invoked from subdirectories, secondary `jj` workspaces (`.workspaces/<name>`), or test harnesses.
   - Resolve relative file paths against an explicit `base_dir: &Path` parameter, discovered VCS/Cargo workspace root, or `env!("CARGO_MANIFEST_DIR")` in build/test fixtures.
   - Validate or canonicalize user/diagnostic paths so `..` segments or symlinks cannot escape the intended target root when writing files.
2. **Centralized, Non-Interactive & Bounded Subprocess Execution:**
   - All `std::process::Command` invocations live in dedicated runner modules (`src/tools/*.rs`), pass arguments as discrete `.arg(...)` elements (never `sh -c` with unescaped string interpolation—preventing command injection), and capture `stdout`/`stderr` cleanly.
   - External tools capable of opening interactive pagers or prompts **must** pass non-interactive flags (e.g., `jj --no-pager ...`, `cargo ... --quiet` / `--message-format=json`), and long-running or agent-under-test processes must enforce explicit timeouts to prevent hung CI/eval runs.
3. **Sandbox & Environment Containment:**
   - Respect read-only host mounts (`CARGO_HOME`, `/nix/store`, `~/.cargo`), write temporary artifacts exclusively into `tempfile::TempDir` or designated writable workspace directories, and never probe host sockets, credentials, or outbound network endpoints when network isolation is active.
   - Secondary Jujutsu workspaces (`.workspaces/*`) lack a `.git` directory; scripts and Nix/VCS helpers must not assume `.git` exists.

### False-Positive Exclusions

- Default CLI argument values (`#[arg(long, default_value = ".")]`) resolved explicitly at the CLI entrypoint into a `PathBuf` passed down as `target_path` are valid user-facing defaults, not implicit CWD bugs.

---

## 4. Subagent Dispatch & Finding Contract

- **Inputs:** Changed Rust/Python/YAML/Nix files (`jj --no-pager diff`) and `code-review purist` output.
- **Output Schema:**
  - `verdict`: `"PASS"` | `"FAIL"`
  - `findings`: List of `{ severity: "CRITICAL" | "MAJOR" | "MINOR", file_span: "<path>:<line>", category: "containment::<cwd_path|unbounded_command|shell_injection|sandbox_escape|env_scatter>", issue: "<containment or hang risk>", fix: "<root-anchored path, centralized arg-safe runner, or --no-pager/timeout>" }`. Empty when compliant.

---

## 5. Playbook Hygiene & Verification

- `skills/reviewing-containment-safety/SKILL.md` starts with YAML frontmatter (`name: reviewing-containment-safety`, `description: Use when...`), stays strictly `< 500` words (`wc -w`), and includes checklist, subagent prompt, rationalization table, and red flags.
- Verified via `prettier --check "**/*.{md,json}"` and `cargo run -p code-review -- check`.
