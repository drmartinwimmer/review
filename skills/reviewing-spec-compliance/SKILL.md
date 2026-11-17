---
name: reviewing-spec-compliance
description: Use when reviewing a pull request, milestone implementation, or task diff against SPEC.md, *.spec.md, plan/M*.md, or PLAN.md to verify requirement completeness, invariant preservation, and absence of out-of-scope feature creep.
---

# Reviewing Spec Compliance

Verify that code changes satisfy every stated specification requirement and invariant—nothing missing, nothing stubbed, and zero unrequested scope creep. Full contracts: [SPEC.md](./SPEC.md).

## Mechanical Pre-Checks

Run before requirement tracing:

1. `jj --no-pager diff --summary` and `jj --no-pager diff` — enumerate every touched file and behavioral delta.
2. `cargo run -p code-review -- check` and `cargo test --all-targets --all-features` — verify baseline build, lint, API manifest, and test gates pass.

## Review Checklist

1. **1:1 Requirement & Invariant Traceability**
   - Map each normative clause in `SPEC.md`, `*.spec.md`, and `plan/M*.md` (CLI flags/aliases, exit codes `0`/`1`/`2`, output formats, idempotency, error variants) to concrete implementation lines and tests.
   - Flag (`CRITICAL`/`MAJOR`) dead/no-op flags, partial stubs, or unverified acceptance criteria.
2. **Strict Scope Containment (Zero Feature Creep)**
   - Reject pre-implemented future milestone features, unrequested CLI flags/config knobs, speculative abstractions, or drive-by refactors of unrelated crates.
3. **Spec, Manifest & Plan Synchronization**
   - Confirm intentional design refinements update `SPEC.md`, `API.md`, `PLAN.md`, and `plan/M*.md` coherently in the same change rather than drifting silently.
4. **Milestone Completion Hygiene**
   - Verify task checkboxes and completion dates in `PLAN.md`, `plan/M<N>.md`, and `plan/FEEDBACK_M<N>.md` match actual delivered artifacts.

## Subagent Prompt & Output Format

```text
Review <target_path_or_diff> against <SPEC.md / plan/M*.md> using skills/reviewing-spec-compliance/SKILL.md.
Trace every spec requirement and invariant to code and tests, check for scope creep, and return:
Verdict: PASS | FAIL
Findings (empty if clean—no speculative nitpicks):
- [CRITICAL|MAJOR|MINOR] <file>:<line> — spec_compliance::<rule>: <issue> | Fix: <remediation>
```

## Common Rationalizations

| Excuse                                                      | Reality                                                                                |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| "The CLI parses `--flag`, wiring it up can wait."           | Dead flags deceive users and violate CLI contracts; implement fully or omit per spec.  |
| "While here, I added extra options for Milestone N+1."      | Unrequested features expand review/API surface and violate milestone containment.      |
| "Tests pass, so every `SPEC.md` invariant must hold."       | Passing happy-path tests miss untested exit codes, aliases, and boundary invariants.   |
| "Implementation diverged slightly, no need to edit SPEC.md" | Silent spec drift breaks downstream milestones and reviewers; keep specs synchronized. |

## Red Flags — STOP

- Acceptance criteria checked `[x]` in `plan/M*.md` without matching implementation or tests.
- New files, crates, or public flags absent from `SPEC.md` and `PLAN.md`.
- Exit-code or output-format behavior contradicting `SPEC.md`.
