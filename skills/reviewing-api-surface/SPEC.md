# Skill Specification: API Surface & Deprecation Review (`reviewing-api-surface`)

## 1. Purpose and Scope

The `reviewing-api-surface` skill (`skills/reviewing-api-surface/SKILL.md`) governs review of public library exports, CLI command/flag trees, HTTP route contracts, configuration file schemas, and API deprecation lifecycles across Rust crates and workspaces.

- **In Scope:** Visibility minimalism (`pub` vs. `pub(crate)` / private), `API.md` manifest intentionality (`code-review api check`), Semantic Versioning impact (`cargo-semver-checks`), `#[deprecated(since = "...", note = "...")]` lifecycle compliance for superseded items, Clap CLI flag/subcommand stability, and Rust API Guidelines (RFC 430) naming/ergonomics.
- **Out of Scope:** Internal module file layout without public export changes (`reviewing-rust-modularity`), internal error handling (`reviewing-rust-robustness`), and third-party crate selection (`reviewing-dependencies`).

---

## 2. Mechanical Tool Synergy

Before semantic review, the reviewer runs and inspects:

1. `cargo run -p code-review -- api check` (or `code-review check`): Detects uncommitted drift against checked-in `API.md` manifests (`api::drift::breaking` as `Error`, `api::drift::addition` as `Warning`).
2. `cargo semver-checks check-release --workspace --baseline-rev origin/main`: Mechanically detects breaking `rustdoc` JSON signature, trait, enum, or type changes relative to `main`.
3. `cargo run -p code-review -- purist`: Surfaces `purist::clap_struct_encapsulation`, `purist::lib_facade_hygiene`, `purist::no_has_trait_prefix`, `purist::no_negative_bool`, `purist::no_trivial_getters_setters`, and `purist::cli_run_consumes_self`.

---

## 3. Review Invariants & Domain Criteria

1. **Least-Privilege Visibility (`pub(crate)` Default):**
   - Internal helpers, parser internals, subprocess wrappers, and intermediate data types must use private or `pub(crate)` visibility unless external downstream callers or binary entrypoints strictly require `pub`.
   - Struct fields on public types (especially Clap parser structs and domain configs) should remain private behind constructors/accessors unless plain-data DTO semantics are explicitly intended.
2. **Checked-In `API.md` Intentionality:**
   - Every addition, modification, or removal in `API.md` must correspond directly to a requirement in `SPEC.md` or the active milestone task. Accidental exports caused by making an internal module `pub mod` must be flagged as `MAJOR`.
   - Regeneration (`code-review api dump`) is valid only after verifying every diff line in `API.md` is intentional.
3. **Deprecation Lifecycle Over Abrupt Removal:**
   - When an established public function, type, method, or CLI flag is superseded in a non-breaking release, the superseded item must remain available with `#[deprecated(since = "x.y.z", note = "use `replacement` instead")]` rather than being silently deleted or left undocumented.
   - Intentional breaking removals require Conventional Commit breaking markers (`feat!:` / `BREAKING CHANGE:`) aligned with `docs/release_pipeline.spec.md`.
4. **Idiomatic Rust & CLI Ergonomics (RFC 430):**
   - Conversion prefixes follow cost conventions (`as_` free borrow, `to_` allocate/convert, `into_` ownership-consuming).
   - Getter names omit redundant `get_` prefixes and `Has` trait prefixes; boolean flags avoid negative names (`--no-foo` / `disable_foo` confusion).
   - Public types do not leak private or unnameable third-party implementation types in signatures unless re-exported intentionally.

### False-Positive Exclusions

- Do not flag `pub` items in binary-only crates (`main.rs` without `lib.rs`) unless `API.md` tracks CLI flags.
- Do not demand `#[deprecated]` on internal `pub(crate)` or private refactors, or when a commit explicitly declares a breaking (`!:`) pre-1.0 change per spec.

---

## 4. Subagent Dispatch & Finding Contract

- **Inputs:** Target path or `jj --no-pager diff` range, crate `API.md` and `SPEC.md` files, and `code-review api check` output.
- **Output Schema:**
  - `verdict`: `"PASS"` | `"FAIL"`
  - `findings`: List of `{ severity: "CRITICAL" | "MAJOR" | "MINOR", file_span: "<path>:<line>", category: "api_surface::<subrule>", issue: "<concise description>", fix: "<idiomatic remediation>" }`. Empty when compliant.

---

## 5. Playbook Hygiene & Verification

- `skills/reviewing-api-surface/SKILL.md` starts with YAML frontmatter (`name: reviewing-api-surface`, `description: Use when...`), stays strictly `< 500` words (`wc -w`), and includes the review checklist, subagent prompt template, rationalization table, and red flags.
- Verified via `prettier --check "**/*.{md,json}"` and `cargo run -p code-review -- check`.
