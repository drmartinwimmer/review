# Skill Specification: Specification & Scope Compliance Review (`reviewing-spec-compliance`)

## 1. Purpose and Scope

The `reviewing-spec-compliance` skill (`skills/reviewing-spec-compliance/SKILL.md`) verifies that an implementation satisfies every stated requirement, invariant, CLI contract, and acceptance criterion in `PLAN.md`, `SPEC.md` (`*.spec.md`), and `plan/M*.md`—without missing edge cases or introducing unrequested scope creep.

- **In Scope:** Requirement-to-implementation traceability, invariant preservation (exit codes, CLI flags/aliases, output schemas, idempotency), milestone boundary adherence (`plan/M*.md`, `PLAN.md`), and absence of gold-plating or silent behavioral drift.
- **Out of Scope:** Stylistic Rust module idioms (`reviewing-rust-modularity`), generic lint formatting (`reviewing-rust-lint-hygiene`), and supply-chain vulnerability scanning (`reviewing-dependencies`), unless explicitly mandated as a spec invariant.

---

## 2. Mechanical Tool Synergy

1. `jj --no-pager diff --summary` and `jj --no-pager diff`: Enumerates every added, modified, or deleted file against the milestone task list.
2. `cargo run -p code-review -- check` & `cargo test --all-targets --all-features`: Confirms baseline build, lint, API manifest, and test gates pass before checking requirement completeness.

---

## 3. Review Invariants & Domain Criteria

1. **1:1 Requirement & Invariant Coverage:**
   - Every normative requirement (`must`, exit code mapping, flag name/alias, error condition, JSON/Markdown field, file path) in the relevant `SPEC.md` and `plan/M*.md` must map to concrete implementation code and at least one verifying test or check.
   - Stubbed or partial implementations (e.g., parsing a CLI flag in `clap` but ignoring it at runtime without spec authorization) must be flagged as `CRITICAL` or `MAJOR`.
2. **Strict Milestone & Task Containment (No Scope Creep):**
   - Changes must not pre-implement future milestones, add speculative configuration knobs, introduce unrequested CLI subcommands, or refactor unrelated crates outside the task scope.
3. **Spec-to-Code Synchronization:**
   - When an implementation legitimately refines a design detail (e.g., extracting an in-tree stub crate into a standalone crate or adjusting a flag alias), `SPEC.md`, `API.md`, `PLAN.md`, and `plan/M*.md` must be updated coherently in the same change rather than drifting silently.
4. **Milestone Artifact Hygiene:**
   - Milestone completions must update task checkboxes in `PLAN.md`, `plan/M<N>.md`, and `plan/FEEDBACK_M<N>.md` accurately.

### False-Positive Exclusions

- Do not penalize concise internal helper functions or error variants required to implement specified behavior cleanly.
- Do not demand features explicitly deferred to later milestones in `PLAN.md`.

---

## 4. Subagent Dispatch & Finding Contract

- **Inputs:** Target milestone/task description (`PLAN.md`, `plan/M*.md`), relevant `SPEC.md` / `*.spec.md` files, and `jj --no-pager diff`.
- **Output Schema:**
  - `verdict`: `"PASS"` | `"FAIL"`
  - `findings`: List of `{ severity: "CRITICAL" | "MAJOR" | "MINOR", file_span: "<path>:<line>", category: "spec_compliance::<missing_requirement|invariant_violation|scope_creep|doc_drift>", issue: "<requirement vs. reality gap>", fix: "<exact implementation or trim needed>" }`. Empty when compliant.

---

## 5. Playbook Hygiene & Verification

- `skills/reviewing-spec-compliance/SKILL.md` starts with YAML frontmatter (`name: reviewing-spec-compliance`, `description: Use when...`), stays strictly `< 500` words (`wc -w`), and includes checklist, subagent prompt, rationalization table, and red flags.
- Verified via `prettier --check "**/*.{md,json}"` and `cargo run -p code-review -- check`.
