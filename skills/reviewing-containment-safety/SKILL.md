---
name: reviewing-containment-safety
description: Use when reviewing filesystem path resolution, std::process::Command execution, environment variable reads, VCS/CLI scripts, or sandbox drivers to prevent CWD path bugs, interactive hangs, command injection, and sandbox escapes.
---

# Reviewing Containment Safety

Enforce root-anchored paths, centralized non-interactive subprocess execution, and strict filesystem/network containment across Rust crates, scripts, and sandboxes. Full contracts: [SPEC.md](./SPEC.md).

## Mechanical Pre-Checks

Run before auditing I/O and subprocess boundaries:

1. `cargo run -p code-review -- purist` — enforces `path_resolution`, `centralized_command_execution`, `no_env_access_outside_config`, and `raii_temp_directories`.
2. `jj --no-pager diff` — inspect every `Path`, `fs::*`, `Command::new`, `env::*`, shell script, and sandbox flag change.

## Review Checklist

1. **Workspace / Root-Anchored Paths (`cwd_path_resolution`)**
   - Never assume process `CWD` equals the crate or repo root; resolve relative paths against an explicit `base_dir: &Path`, discovered workspace root, or `env!("CARGO_MANIFEST_DIR")`.
   - Canonicalize or validate externally supplied paths so `..` segments or symlinks cannot escape target directories on write.
2. **Centralized, Injection-Safe & Non-Interactive Subprocesses**
   - Isolate `std::process::Command::new` inside dedicated `src/tools/*.rs` modules (`centralized_command_execution`) and pass discrete `.arg(...)` values—never `sh -c` string interpolation.
   - Always pass non-interactive flags (`jj --no-pager ...`, `--message-format=json`, `--quiet`) and enforce explicit timeouts on external/agent processes so runs never hang on TTY prompts or pagers.
3. **Centralized Environment & Secondary Workspace Compatibility**
   - Read environment variables only at configuration/CLI boundaries (`no_env_access_outside_config`).
   - Do not assume `.git` exists (secondary `jj` workspaces under `.workspaces/*` have `.jj` only; Nix commands require `path:.` or flake staging).
4. **Sandbox & Host Isolation (`fence` / CI)**
   - Keep host mounts read-only, restrict writes to `tempfile::TempDir` or ephemeral eval directories, respect read-only `CARGO_HOME`/`/nix/store`, and block outbound network access in isolated evals.

## Subagent Prompt & Output Format

```text
Review path handling, subprocesses, env access, and sandbox bounds in <target_path_or_diff> using skills/reviewing-containment-safety/SKILL.md.
Run `code-review purist`, inspect every Path/fs/Command/env site, and return:
Verdict: PASS | FAIL
Findings (empty if clean—no speculative nitpicks):
- [CRITICAL|MAJOR|MINOR] <file>:<line> — containment::<rule>: <issue> | Fix: <remediation>
```

## Common Rationalizations

| Excuse                                                      | Reality                                                                                  |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| "`fs::read_to_string(\"Cargo.toml\")` works from the root." | Breaks from subdirectories, tests, and workspaces; join against an explicit `base_dir`.  |
| "It's one tiny `Command::new` call in `command.rs`."        | Scatters process execution and bypasses mockable `tools/` runners.                       |
| "`jj diff` doesn't page in non-TTY pipes anyway."           | Pseudo-TTY harnesses hang indefinitely; always pass `jj --no-pager`.                     |
| "`sh -c \"tool $path\"` is shorter than `.args(...)`."      | Breaks on spaces/metacharacters and enables shell injection; pass discrete `.arg(path)`. |

## Red Flags — STOP

- Bare relative `Path::new("...")` / `fs::*("...")` without an explicit base directory.
- `Command::new` outside `src/tools/`, `sh -c` interpolation, or missing `--no-pager` / timeouts.
- Assuming `.git` exists, writing outside ephemeral/target roots, or bypassing sandbox network/fs bounds.
