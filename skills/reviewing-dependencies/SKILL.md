---
name: reviewing-dependencies
description: Use when reviewing Cargo.toml or Cargo.lock changes, proposed crate additions, version bumps, or feature flags to enforce YAGNI minimalism, established ecosystem choices, supply-chain security, and permissive licensing.
---

# Reviewing Dependencies

Keep the dependency graph minimal, standard, audited, and MIT-compatible. Every new crate is permanent compile-time and supply-chain liability. Full contracts: [SPEC.md](./SPEC.md).

## Mechanical Pre-Checks

Run before qualitative review:

1. `cargo audit` (or `cargo run -p code-review -- check`) — RustSec CVEs, yanked versions, unmaintained warnings.
2. `CARGO_BUILD_WARNINGS=deny cargo clippy --all-targets --all-features -- -D warnings` — enforces `cargo::unused_dependencies` and `cargo::unused_workspace_dependencies`.
3. `cargo info <crate>` and `cargo tree -p <crate>` (or `cargo tree -i <crate>`) — metadata, license, downloads/adoption, and transitive weight.

## Review Checklist

1. **Necessity & YAGNI (`std` First)**
   - Reject crates solvable in ~10–30 lines of Rust `std` (string/path helpers, basic retries, env parsing, simple prefix matching without `regex`, sync CLI tasks without `tokio`).
   - Reject duplicates of existing `[workspace.dependencies]` (`clap`, `serde_json`, `toml_edit`, `thiserror`, `tempfile`, `googletest`) and trim unneeded `default-features`.
2. **Popularity, Maintenance & Established Standards**
   - Inspect `cargo info <crate>` (downloads, reverse dependencies, release recency, repository health); flag abandoned or single-maintainer hobby crates (`obscure_deps`) when de facto standards (`clap`, `serde`, `thiserror`/`anyhow`, `tracing`) exist.
3. **Security Advisories & Typosquatting**
   - Block (`CRITICAL`) any RustSec CVE, unsound/unmaintained alert, yanked release, or look-alike package name (`serde_josn`, `clapp`).
4. **Transitive Weight & Build-Time Execution Risks**
   - Audit `cargo tree` depth/breadth; scrutinize `build.rs` scripts, native `-sys` bindings, and heavy proc-macros running code at compile time; prefer pure-Rust, `unsafe`-free crates.
5. **License Compatibility**
   - Require MIT/Apache-2.0-compatible permissive licenses (`MIT`, `Apache-2.0`, `BSD-*`, `ISC`, `Unicode-3.0`); block (`CRITICAL`) copyleft (`GPL-*`, `AGPL-*`, `LGPL-*`) or unlicensed crates.

## Subagent Prompt & Output Format

```text
Review Cargo.toml and Cargo.lock changes in <target_path_or_diff> using skills/reviewing-dependencies/SKILL.md.
Run `cargo audit`, `cargo info`, and `cargo tree` on added/updated crates and return:
Verdict: PASS | FAIL
Findings (empty if clean—no speculative nitpicks):
- [CRITICAL|MAJOR|MINOR] Cargo.toml:<line> — dependencies::<rule>: <issue> | Fix: <remediation>
```

## Common Rationalizations

| Excuse                                                  | Reality                                                                                   |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| "This tiny helper crate saves writing 15 lines."        | 15 lines of `std` have zero CVEs, `build.rs` hooks, or transitive churn.                  |
| "This niche crate has a nicer API than `clap`/`serde`." | Obscure crates carry abandonment and audit risk; use established ecosystem standards.     |
| "Default features don't hurt."                          | Default features pull TLS stacks, codecs, and proc-macros; enable only required features. |
| "It's only LGPL/GPL in a transitive sub-crate."         | Copyleft contamination violates MIT distribution guarantees—reject immediately.           |

## Red Flags — STOP

- Adding async runtimes, HTTP stacks, or regex engines for synchronous local CLI logic.
- Unresolved `cargo audit` advisories, yanked crates, or unverified `build.rs` network/shell scripts.
- Missing `license` metadata or any `GPL`/`AGPL` expression in `cargo tree -f "{p} {l}"`.
