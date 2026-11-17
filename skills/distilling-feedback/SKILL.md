---
name: distilling-feedback
description: Use when analyzing past agent session logs (transcript.jsonl), user course corrections, Jujutsu fixup commits, PR review comments, or milestone feedback files to distill recurring agent failures into mechanical linter rules, review guidelines, and evaluation test cases.
---

# Distilling Feedback

Convert concrete agent mistakes and human corrections into permanent defenses: **mechanical CLI/AST checks first**, **review skill rules second**, and **minimal Inspect AI eval fixtures third**. Full schemas and contracts: [SPEC.md](./SPEC.md).

## Workflow

1. **Scan Read-Only**
   - Read `<appDataDir>/brain/<id>/.system_generated/logs/transcript.jsonl` first; inspect matching lines in `transcript_full.jsonl` **only** when `truncated_fields` is set.
   - Grep `"invoke_subagent"` to trace child `[subagent](conversation://<id>)` logs; inspect `jj --no-pager log`, `jj --no-pager evolog`, `plan/FEEDBACK_M*.md`, and PR reviews.
2. **Extract Signals & Excuses**
   - Flag `USER_INPUT` overrides (`"stop"`, `"revert"`, `"don't"`, `"undo"`, `"instead"`, `"why did you"`), `status == "ERROR"` loops, pager hangs, and `#[allow]`/`#[expect]` shortcuts.
   - Capture `conversation://<id>` + `step_index`, the technical defect, and the preceding `PLANNER_RESPONSE.thinking` excuse.
3. **Classify & Route (Mechanical First)**
   - Prefer deterministic rules (`purist`, `configure-lints`, `api`, `coverage`, `check`) over prose whenever AST/CLI-checkable:
   - `modularity` (inline `mod`, unit-struct namespaces, fat `lib.rs`) -> `purist`, `reviewing-rust-modularity`, `inline_mods/`
   - `robustness` (`.unwrap()`, slicing, `Result<T, String>`, ignored results) -> `configure-lints`/`purist`, `reviewing-rust-robustness`, `unwrap_panics/`
   - `lint_hygiene` (unjustified `#[allow]`/`#[expect]`, warnings) -> `configure-lints`/`purist`, `reviewing-rust-lint-hygiene`, `unjustified_suppression/`
   - `testing_coverage` (`assert_eq!`, test `.unwrap()`, leaks, coverage gaps) -> `coverage`/`purist`, `reviewing-rust-testing`, `{leaky_tests,coverage_gaps}/`
   - `containment_safety` (CWD paths, raw `Command::new`, pager hangs, boundary bugs) -> `purist`, `reviewing-containment-safety`, `cwd_path_resolution/`
   - `api_surface` (`pub` leaks, `API.md` drift, missing `#[deprecated]`) -> `api`/`cargo-semver-checks`, `reviewing-api-surface`, `{api_drift,missing_deprecations}/`
   - `dependencies` (YAGNI bloat, obscure/unmaintained crates, CVEs, copyleft) -> `check`, `reviewing-dependencies`, `{dependency_bloat,obscure_deps,vulnerable_deps}/`
   - `spec_compliance` (scope creep, missed `SPEC.md` invariants) -> `check`, `reviewing-spec-compliance`, `clean_baseline/`
4. **Synthesize Artifacts**
   - **Mechanical Rule Spec:** `<tool>::<rule_id>`, AST/CLI pattern, false-positive exclusions, `suggested_fix`.
   - **Skill Guideline Patch** (+ optional `<=10`-line Bad/Good Rust snippet):
     ```markdown
     ### <Imperative Rule Title>

     - **Check:** <1-sentence condition> (`[ref](conversation://<id>)` step `<n>`)

     | Excuse                                | Reality                          |
     | ------------------------------------- | -------------------------------- |
     | "<Observed thinking rationalization>" | <Why it fails + required action> |

     - **Red Flag:** <Immediate stop trigger>
     ```
   - **Eval Case (`evals/datasets/<category>/<case_id>/`):** Sanitize secrets, paths, and domain names into a self-contained `<60`-line single-defect crate with ground-truth `defect_id`, span, and fix.

## Common Rationalizations

| Excuse                                              | Reality                                                                                |
| --------------------------------------------------- | -------------------------------------------------------------------------------------- |
| "I'll read `transcript_full.jsonl` first."          | Wastes context; scan `transcript.jsonl` and dereference only `truncated_fields` lines. |
| "Prose in `SKILL.md` is faster than a linter spec." | Prose drifts; AST-checkable defects belong in `purist` or `configure-lints`.           |
| "Didn't happen yet, but worth adding."              | Zero speculative bloat—every rule requires a concrete incident citation.               |
| "I'll copy the real file into `evals/datasets/`."   | Real files leak context and mix flaws; reduce to a sanitized single-defect crate.      |

## Red Flags — STOP

- Proposing rules without a `conversation://<id>` step or commit citation.
- Writing multi-paragraph prose instead of a check, excuse counter, or AST rule.
- Omitting `--no-pager` on `jj` commands or mutating `<appDataDir>/brain/` logs.
