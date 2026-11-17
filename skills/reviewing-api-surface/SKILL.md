---
name: reviewing-api-surface
description: Use when reviewing public Rust library items, CLI flags or subcommands, HTTP endpoints, configuration schemas, or API.md diffs to verify visibility minimalism, SemVer compatibility, and deprecation lifecycles.
---

# Reviewing API Surface

Audit every exported Rust symbol, CLI flag, HTTP route, and `API.md` diff for least-privilege visibility, intentionality, idiomatic ergonomics, and clean deprecation lifecycles. Full contracts: [SPEC.md](./SPEC.md).

## Mechanical Pre-Checks

Run before semantic review:

1. `cargo run -p code-review -- api check` (`api::drift::breaking`, `api::drift::addition`)
2. `cargo run -p code-review -- purist` (`clap_struct_encapsulation`, `lib_facade_hygiene`, `no_has_trait_prefix`, `no_negative_bool`, `no_trivial_getters_setters`, `cli_run_consumes_self`)
3. `cargo semver-checks check-release --workspace --baseline-rev origin/main`

## Review Checklist

1. **API Minimalism (`pub(crate)` Default)**
   - Internal helpers, parsers, tool runners, and intermediate structs use private or `pub(crate)` visibility unless external callers strictly require `pub`.
   - Public and `clap::Parser` structs keep fields private behind constructors/accessors unless plain DTOs by spec.
2. **Checked-In `API.md` Intentionality**
   - `API.md` has zero uncommitted drift and every added/modified/removed entry traces directly to `SPEC.md` or task scope (never accidental `pub mod` leaks).
3. **Deprecation Lifecycle & SemVer Governance**
   - Superseded public functions, types, or CLI flags retain backwards compatibility with `#[deprecated(since = "x.y.z", note = "use \`replacement\` instead")]` rather than silent deletion.
   - Intentional breaking removals require `feat!:` / `BREAKING CHANGE:` commit annotations.
4. **RFC 430 Naming & Ergonomics**
   - Conversion methods follow `as_` (free borrow), `to_` (allocating/converting), `into_` (consuming); getters omit `get_` and traits omit `Has`; booleans use positive names; signatures avoid leaking implementation-only crate types.

## Subagent Prompt & Output Format

```text
Review <target_path_or_diff> using skills/reviewing-api-surface/SKILL.md.
Run `code-review api check` and `code-review purist`, inspect API.md and pub items, and return:
Verdict: PASS | FAIL
Findings (empty if clean—no speculative nitpicks):
- [CRITICAL|MAJOR|MINOR] <file>:<line> — api_surface::<rule>: <issue> | Fix: <remediation>
```

## Common Rationalizations

| Excuse                                                     | Reality                                                                                |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| "Making it `pub` is convenient for tests/submodules."      | Use `pub(crate)` or `pub(super)`; `pub` locks in SemVer obligations and bloats API.md. |
| "I ran `code-review api dump` so the check passes."        | Dumping without auditing each `API.md` diff rubber-stamps accidental API leaks.        |
| "Nobody uses the old function yet, just delete it."        | Published/documented APIs require `#[deprecated]` or an explicit `feat!:` bump.        |
| "`pub` fields on `clap::Parser` structs save boilerplate." | Exposes CLI parsing internals as public API; keep fields private with accessors.       |

## Red Flags — STOP

- Uncommitted `API.md` drift or blind `api dump` containing unrelated `pub` helpers.
- Removed or renamed `pub` items / CLI flags lacking `#[deprecated]` and lacking `!:`.
- `pub mod` exposing internal implementation modules at the crate root.
