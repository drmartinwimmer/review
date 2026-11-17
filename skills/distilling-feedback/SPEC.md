# Skill Specification: Feedback Reflection & Guideline Distillation (`distilling-feedback`)

## 1. Purpose and Overview

The `distilling-feedback` skill (`skills/distilling-feedback/SKILL.md`) defines a repeatable, evidence-driven workflow for mining past agent session transcripts, user course corrections, version-control fixups, and review comments to harden the `code-review` ecosystem against recurring agent failure modes.

Rather than treating user corrections as one-off fixes, `distilling-feedback` closes the meta-learning loop across three defense layers:

1. **Mechanical Static Analysis & CLI Gates (Highest Leverage):** Deterministic checks in `code-review purist` (`purist`), `code-review configure-lints`, `code-review api` (`api-manifest`), `code-review coverage`, `code-review check`, and `cargo-semver-checks`.
2. **Thematic Subagent Review Skills:** Concise checklist items, rationalization counters, red flags, and minimal before/after examples in `skills/reviewing-*/SKILL.md` for semantic or architectural issues that require judgment.
3. **Inspect AI Evaluation Datasets:** Sanitized, single-defect reproduction crates under `evals/datasets/<category>/` (paired with clean baselines) to measure review recall, precision, and tool synergy over time.

---

## 2. Input Sources & Transcript Schemas

### 2.1 Antigravity Session Transcripts (`transcript.jsonl` & `transcript_full.jsonl`)

Antigravity stores structured session logs per conversation at:

```text
<appDataDir>/brain/<conversation-id>/.system_generated/logs/transcript.jsonl
<appDataDir>/brain/<conversation-id>/.system_generated/logs/transcript_full.jsonl
```

Each line is a JSON object representing a single trajectory step:

| Field              | Type             | Description                                                                                                               |
| ------------------ | ---------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `step_index`       | `integer`        | Zero-based sequential index of the step in the conversation trajectory.                                                   |
| `source`           | `string`         | Originator of the step (`"USER_EXPLICIT"`, `"MODEL"`, `"SYSTEM"`).                                                        |
| `type`             | `string`         | Step kind; primary targets are `"USER_INPUT"` (human prompts/interventions) and `"PLANNER_RESPONSE"` (reasoning + tools). |
| `status`           | `string`         | Execution outcome (`"DONE"`, `"ERROR"`, `"CANCELLED"`).                                                                   |
| `created_at`       | `string`         | ISO 8601 timestamp when the step was recorded.                                                                            |
| `content`          | `string`         | User prompt text, model response text, or serialized tool execution output.                                               |
| `thinking`         | `string` (opt)   | Internal chain-of-thought on `"PLANNER_RESPONSE"` steps; primary source for extracting agent rationalizations/shortcuts.  |
| `tool_calls`       | `array` (opt)    | Invoked tools and arguments (e.g., `run_command`, `edit_file`, `write_file`, `invoke_subagent`).                          |
| `media`            | `array` (opt)    | Attached media descriptors (`{ mime_type, uri }`); bytes live on disk at `uri`.                                           |
| `truncated_fields` | `string[]` (opt) | Present only in `transcript.jsonl` when fields (`"content"`, `"thinking"`, `"tool_calls"`) were compacted.                |

**Progressive Transcript Reading Protocol:**

1. Always scan the compact `transcript.jsonl` first.
2. Read `transcript_full.jsonl` **only** at the specific 1-based line numbers where `truncated_fields` is present and full content is required to diagnose an incident.
3. Discover delegated subagent trajectories by grepping `"invoke_subagent"` in `transcript.jsonl`, extracting child `<conversation-id>` values, and inspecting their corresponding `transcript.jsonl` files (referenced in reports via `[label](conversation://<conversation-id>)`).

### 2.2 Generic / CLI Agent JSONL Transcripts

When analyzing sessions from other JSONL-emitting coding agents (e.g., Claude Code `~/.claude/projects/**/*.jsonl` or Inspect AI logs), normalize records onto the same logical fields:

- **User turns:** `role == "user"` text blocks (excluding automated tool-result wrappers).
- **Model turns & rationalizations:** `role == "assistant"` `thinking` and `tool_use` blocks.
- **Tool failures:** `tool_result` blocks where `is_error == true` or command exit codes are non-zero.

### 2.3 VCS History & Asynchronous Review Artifacts

Transcript analysis is supplemented by:

- **Jujutsu Revision & Evolution Logs:** `jj --no-pager log`, `jj --no-pager evolog`, and `jj --no-pager diff` to identify manual human fixup commits, backed-out revisions, or post-PR remediation commits.
- **Milestone Feedback Files:** `plan/FEEDBACK_M*.md` (`Issues & Concerns`, `Suggestions & Follow-ups`, `Action Items`).
- **Pull Request Reviews:** Inline review comments and requested changes on GitHub PRs.

---

## 3. Signal Extraction & Incident Triage

An incident candidate is recorded whenever one or more of the following signals appear:

1. **Explicit User Course Corrections (`USER_INPUT`):**
   - Directive overrides or frustration markers: `"stop"`, `"revert"`, `"undo"`, `"don't"`, `"do not"`, `"no,"`, `"wrong"`, `"why did you"`, `"instead"`, `"I asked for"`, `"you broke"`, `"unused"`, `"too complicated"`.
2. **Tool Execution Failures & Loops:**
   - Steps with `status == "ERROR"` or repeated non-zero exit codes (`cargo clippy`, `cargo test`, `purist`, `prettier`, `taplo`, `nix flake check`).
   - Hung or killed interactive commands (e.g., running `jj log`/`jj diff` without `--no-pager`, or invoking raw `git` commands in a `jj` workspace).
3. **Quality Gate Evasion & Shortcut Signals:**
   - Adding `#[allow(...)]` or `#[expect(...)]` without a concrete justification comment or `reason = "..."`.
   - Passing `--skip-*` / `--no-verify` to silence failing gates instead of fixing the underlying defect.
   - Weakening tests, deleting assertions, or leaving error branches untested.
4. **Human Remediation Diffs:**
   - Commits authored directly by the human user immediately following an agent revision that tighten visibility (`pub` -> `pub(crate)`), remove bloat dependencies, split monolithic files, or fix edge cases.

For every confirmed incident, the analyst extracts a **Root-Cause Tuple**:

- **Evidence Reference:** `conversation://<id>` + `step_index` (or commit/PR reference).
- **Observed Defect:** Concrete code, command, or process mistake.
- **Agent Rationalization:** The excuse or flawed assumption in `thinking` that preceded the mistake (e.g., _"I'll just add `#[allow(clippy::unwrap_used)]` here since we checked length earlier"_).
- **Corrective Action:** What actually resolved the issue.

---

## 4. Failure Taxonomy & Defense Routing

Every root-caused incident must be classified into one primary category and routed to its corresponding mechanical tool, review skill, and evaluation dataset directory:

| Category ID          | Failure Patterns                                                                                       | Mechanical Tool Target                                                          | Thematic Skill Target                 | Eval Dataset Target (`evals/datasets/`)                  |
| -------------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- | ------------------------------------- | -------------------------------------------------------- |
| `modularity`         | Inline `mod foo { ... }` in `lib.rs`/`main.rs`, dummy unit-struct namespaces, bloated `lib.rs` facade  | `code-review purist` (`no_inline_mods`, `free_functions`, `lib_facade_hygiene`) | `skills/reviewing-rust-modularity`    | `inline_mods/`                                           |
| `robustness`         | `.unwrap()`, indexing/slicing panics, `Result<T, String>`, ignored `#[must_use]` / `let _ = ...`       | `code-review configure-lints` & `code-review purist` (`error_types`)            | `skills/reviewing-rust-robustness`    | `unwrap_panics/`                                         |
| `lint_hygiene`       | Unjustified `#[allow]`/`#[expect]`, compiler/Cargo manifest warnings, skipped lint gates               | `code-review configure-lints` & `code-review purist` (`clippy_suppress`)        | `skills/reviewing-rust-lint-hygiene`  | `unjustified_suppression/`                               |
| `testing_coverage`   | Raw `assert_eq!` over `expect_that!`, `.unwrap()` in tests, leaked temp state, uncovered error paths   | `code-review coverage` & `code-review purist` (`test_patterns`)                 | `skills/reviewing-rust-testing`       | `leaky_tests/`, `coverage_gaps/`                         |
| `containment_safety` | CWD-relative file paths, scattered `Command::new`, missing `--no-pager`, sandbox/network boundary bugs | `code-review purist` (`path_resolution`, `centralized_command_execution`)       | `skills/reviewing-containment-safety` | `cwd_path_resolution/`                                   |
| `api_surface`        | Leaked `pub` internals (`pub(crate)` suffices), uncommitted `API.md` drift, missing `#[deprecated]`    | `code-review api` (`api-manifest`) & `cargo-semver-checks`                      | `skills/reviewing-api-surface`        | `api_drift/`, `missing_deprecations/`                    |
| `dependencies`       | Heavy crates for stdlib tasks (YAGNI), obscure/unmaintained crates, CVEs, `build.rs` or license risks  | `code-review check` (`cargo audit`, `cargo::unused_dependencies`)               | `skills/reviewing-dependencies`       | `dependency_bloat/`, `obscure_deps/`, `vulnerable_deps/` |
| `spec_compliance`    | Out-of-scope feature creep, missed `SPEC.md` invariants, unrequested UX/API changes                    | `code-review check` (manifest/schema gates where applicable)                    | `skills/reviewing-spec-compliance`    | `clean_baseline/` (paired false-positive checks)         |

---

## 5. Invariants and Architectural Guarantees

1. **Evidence-Grounded Distillation (Zero Speculative Bloat):**
   - Every distilled guideline, linter rule proposal, and eval fixture must cite at least one concrete historical incident (`conversation-id` + `step_index`, Jujutsu change/commit ID, or `FEEDBACK_M*.md` item). Hypothetical rules without observed failures must not be added.
2. **Mechanical Enforcement Precedence:**
   - If a recurring defect can be detected deterministically via `syn` AST inspection, Clippy/Cargo lint configuration, `api-manifest`, or `cargo-llvm-cov`, the distillation output **must** propose a mechanical rule first and keep any accompanying skill guidance minimal. Prose checklists are reserved for semantic nuances that static analysis cannot decide cleanly.
3. **Sanitization & Minimal Abstraction:**
   - All outputs must strip API keys, tokens, user-specific absolute paths, proprietary domain names, and incidental business logic.
   - Extracted `evals/datasets/` fixtures must be minimal, self-contained Rust crates (`Cargo.toml` + `src/lib.rs` or `src/main.rs`, typically `< 60` lines) isolating a single defect class so evaluation failures remain unambiguous.
4. **Read-Only Transcript & VCS Inspection:**
   - The distillation workflow never mutates session logs under `<appDataDir>/brain/` and always passes `--no-pager` to Jujutsu inspection commands (`jj --no-pager log`, `jj --no-pager diff`).
5. **Skill Playbook Hygiene & Token Efficiency:**
   - `skills/distilling-feedback/SKILL.md` must begin with YAML frontmatter containing `name: distilling-feedback` and a `description` starting with `Use when...` (stating triggering conditions only, without summarizing internal steps).
   - `skills/distilling-feedback/SKILL.md` must remain under `500` words (`wc -w < 500`), include a rationalization table (`Excuse | Reality`) and explicit `Red Flags`, and link to `./SPEC.md` for full schemas.

---

## 6. Output Artifact Schemas

A complete execution of `distilling-feedback` produces a structured **Distillation Report** containing up to three actionable artifact sections:

### 6.1 Skill Guideline Entry (for `skills/reviewing-*/SKILL.md`)

```markdown
### <Concise Imperative Rule Title>

- **Check:** <1-sentence verifiable condition>
- **Evidence:** `[session](conversation://<id>)` step `<n>` / commit `<rev>`

| Excuse (Rationalization)                            | Reality                               |
| --------------------------------------------------- | ------------------------------------- |
| "<Exact excuse pattern observed in model thinking>" | <Why it fails and what to do instead> |

- **Red Flag:** <Observable code or thought smell that requires immediate stop>
```

Accompanied by a minimal (`<= 10` lines each) **Bad / Good** Rust snippet pair when syntax patterns clarify intent.

### 6.2 Mechanical Linter / Tool Rule Proposal

```markdown
### Rule Proposal: `<tool>::<rule_id>`

- **Target Tool:** `purist` | `configure-lints` | `api-manifest` | `coverage` | `check`
- **Severity:** `Error` | `Warning`
- **Detection Pattern:** <Exact AST node / Cargo.toml lint / CLI output condition>
- **False-Positive Exclusions:** <Explicit exemptions, e.g., `#[cfg(test)]` modules or main entrypoints>
- **Suggested Fix:** <Actionable remediation message emitted in `Diagnostic::suggested_fix`>
- **Evidence:** <Transcript step / revision references>
```

### 6.3 Inspect AI Evaluation Case Specification (`evals/datasets/<category>/<case_id>/`)

```markdown
### Eval Case: `<category>/<case_id>`

- **Target Skill(s) & Tool(s):** `reviewing-<domain>`, `code-review <subcommand>`
- **Crate Files:** Minimal `Cargo.toml` + `src/lib.rs` (abstracted, zero external secrets/context)
- **Ground-Truth Annotation:**
  - `defect_id`: `<category>::<specific_flaw>`
  - `location`: `src/lib.rs:<start_line>-<end_line>`
  - `expected_finding`: <What the reviewing agent must flag>
  - `expected_fix`: <Idiomatic Rust remediation>
- **Clean Counterpart Note:** <Minimal change turning the crate into a zero-defect precision check>
```

---

## 7. Verification Strategy

1. **Frontmatter & Word-Count Gate:**
   - Verify `skills/distilling-feedback/SKILL.md` contains valid `---` YAML frontmatter with `name: distilling-feedback` and `description: Use when...`.
   - Verify `wc -w skills/distilling-feedback/SKILL.md` is strictly `< 500` words.
2. **Formatting & Link Integrity:**
   - Verify relative links between `SKILL.md` and `SPEC.md` resolve cleanly.
   - Run `prettier --check "**/*.{md,json}"` across all created and updated Markdown files.
3. **Workspace Quality Gate:**
   - Run `cargo run -p code-review -- check` and `cargo test --all-targets --all-features` with zero warnings or errors.
