# Specification: Release Pipeline & Automated SemVer Verification

## 1. Purpose and Overview

The release and Semantic Versioning (SemVer) pipeline governs how workspace crates in `review` are versioned, verified, tagged, and published to `crates.io` and GitHub Releases, while keeping the Nix flake package (`flake.nix`) and Cargo workspace metadata (`Cargo.toml`, `Cargo.lock`, and `API.md` manifests) synchronized.

The pipeline implements a closed **Intent vs. Reality** verification loop:

1. **Developer Intent (Release Please):** Parses [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `feat!:`, `BREAKING CHANGE:`) merged into `main` to calculate target pre-1.0 (`0.x`) version bumps, generate per-crate `CHANGELOG.md` entries, and maintain a unified Release Candidate PR.
2. **Code Reality (`cargo-semver-checks`):** Analyzes compiled `rustdoc` JSON across all publishable library crates to mechanically verify that public API changes satisfy Cargo's SemVer compatibility rules:
   - **On Feature PRs:** Compares `HEAD` against `origin/main` to block unannotated breaking public API changes before merge.
   - **On Release Candidate PRs:** Compares the bumped `Cargo.toml` candidate versions against published `crates.io` baselines (or prior release tags) to verify that the version bump computed by Release Please legally covers all accumulated API changes before publishing.

---

## 2. Workspace Package Topology

All four workspace crates are published to `crates.io` in topological dependency order:

| Crate Path               | Package Name                  | Binary Target     | Internal Workspace Dependencies                                            | Synced Extra Files                                                                       |
| ------------------------ | ----------------------------- | ----------------- | -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `crates/configure-lints` | `code-review-configure-lints` | `configure-lints` | None (leaf crate)                                                          | `/Cargo.toml` (`$.workspace.dependencies['code-review-configure-lints'].version`)        |
| `crates/coverage`        | `code-review-coverage`        | `coverage`        | None (leaf crate)                                                          | `/Cargo.toml` (`$.workspace.dependencies['code-review-coverage'].version`)               |
| `crates/check`           | `code-review-check`           | `check`           | None (leaf crate)                                                          | `/Cargo.toml` (`$.workspace.dependencies['code-review-check'].version`)                  |
| `crates/code-review`     | `code-review`                 | `code-review`     | `code-review-configure-lints`, `code-review-coverage`, `code-review-check` | `/Cargo.toml` (`$.workspace.package.version`), `/flake.nix` (`x-release-please-version`) |

---

## 3. Conventional Commits & Pre-1.0 (`0.x`) SemVer Contract

While crates remain in initial development (`0.y.z`, `y >= 1`), Cargo treats the left-most non-zero component (`y`) as the major/breaking version slot and `z` as the SemVer-compatible minor/patch slot (`^0.2.0` matches `>=0.2.0, <0.3.0`).

To align Release Please with Cargo's `0.x` semantics and `cargo-semver-checks`, `release-please-config.json` enforces:

- `"bump-minor-pre-major": true`
- `"bump-patch-for-minor-pre-major": true`
- `"separate-pull-requests": false`
- `"plugins": [{ "type": "linked-versions", ... }]` (synchronizing all four workspace crates at lockstep versions)

### Commit-to-Version Mapping (`0.y.z`)

| Commit Pattern                                    | API Change Compatibility      | `0.y.z` Version Bump         | `>=1.0.0` Version Bump       | `cargo-semver-checks` Expectation |
| ------------------------------------------------- | ----------------------------- | ---------------------------- | ---------------------------- | --------------------------------- |
| `feat!:` / `<type>(scope)!:` / `BREAKING CHANGE:` | Breaking public API change    | **Minor** (`0.2.0 -> 0.3.0`) | **Major** (`1.2.0 -> 2.0.0`) | `--release-type major`            |
| `feat:` / `feat(scope):`                          | Backwards-compatible addition | **Patch** (`0.2.0 -> 0.2.1`) | **Minor** (`1.2.0 -> 1.3.0`) | Compatible (non-breaking)         |
| `fix:` / `fix(scope):`                            | Backwards-compatible bug fix  | **Patch** (`0.2.0 -> 0.2.1`) | **Patch** (`1.2.0 -> 1.2.1`) | Compatible (non-breaking)         |
| `docs:`, `ci:`, `chore:`, `refactor:`, `test:`    | No breaking API change        | No release triggered alone   | No release triggered alone   | Compatible (non-breaking)         |

---

## 4. Invariants and Architectural Guarantees

1. **Single Unified Release PR (`separate-pull-requests: false`):**
   - Because all crates share a virtual root `Cargo.toml` and workspace `Cargo.lock`, Release Please must maintain a single consolidated Release PR (`release-please--branches--main`) rather than competing per-crate PRs that would conflict on `Cargo.lock`.
2. **Virtual Workspace Compatibility (No `cargo-workspace` Plugin):**
   - Release Please's built-in `cargo-workspace` plugin assumes a root `[package]` manifest and fails on virtual Cargo workspaces. Instead, cross-file version propagation uses root-relative `extra-files` entries (`path: "/Cargo.toml"` with `jsonpath`, and `path: "/flake.nix"` with `type: "generic"` and `# x-release-please-version`) paired with a post-PR synchronization step in `.github/workflows/release.yml`.
3. **Lockfile Synchronization on Release PRs:**
   - Whenever Release Please opens or updates a Release Candidate PR (`steps.release.outputs.pr`), the release workflow checks out the PR branch, runs `cargo update --workspace` to refresh workspace member versions in `Cargo.lock`, and commits `Cargo.lock` if modified.
4. **Topological Workspace Crates.io Publication:**
   - Because all four workspace crates advance in lockstep via `linked-versions`, publishing invokes a single `cargo publish --workspace`, which natively resolves topological dependency ordering and waits for `crates.io` sparse-index propagation between leaf crates and `code-review`.
5. **Unannotated Breaking Change Prevention (Feature PR Gate):**
   - Every non-Release-Please PR targeting `main` runs `cargo semver-checks check-release --workspace --baseline-rev origin/main`.
   - Breaking API changes are rejected unless the PR title or a commit message in `origin/main..HEAD` explicitly contains a Conventional Commit breaking marker (`^[a-z]+(\([^)]+\))?!:` or `^BREAKING[ -]CHANGE:`), in which case `--release-type major` is passed.
6. **Release Candidate Bump Sufficiency Gate:**
   - Once initial versions are published on `crates.io`, every Release Please PR (`startsWith(github.head_ref, 'release-please--')`) runs `cargo semver-checks check-release --workspace` against the published `crates.io` baselines.

---

## 5. CI & Release Workflow Architecture

### 5.1 Release Workflow (`.github/workflows/release.yml`)

- **Triggers:** `push` to `main` and manual `workflow_dispatch`.
- **Permissions:** `contents: write`, `pull-requests: write`.
- **Concurrency:** `group: release-${{ github.ref }}`, `cancel-in-progress: false` (never cancel an in-flight `cargo publish`).
- **Execution Phases:**
  1. Execute `googleapis/release-please-action@v5` with `release-please-config.json` and `.release-please-manifest.json`.
  2. If `steps.release.outputs.pr` is populated, check out the Release PR branch and synchronize `Cargo.lock` via `cargo update --workspace`.
  3. If `steps.release.outputs.releases_created == 'true'` or `github.event_name == 'workflow_dispatch'`, validate `CARGO_REGISTRY_TOKEN` and invoke multi-package `cargo publish`.

### 5.2 SemVer Verification Job (`.github/workflows/ci.yml`)

- **Job ID:** `semver` (`SemVer API Compatibility`), running on `pull_request` events targeting `main` and required by `auto-merge`.
- **Toolchain:** Stable Rust toolchain (`dtolnay/rust-toolchain@stable`), `Swatinem/rust-cache@v2`, and `taiki-e/install-action@v2` (`tool: cargo-semver-checks`).
- **Checkout:** `actions/checkout@v7` with `fetch-depth: 0` so `origin/main`, commit ranges, and release tags are locally resolvable.

---

## 6. Verification Strategy

1. **Formatting & Schema Validation:**
   - `prettier --check "**/*.{md,json}"` validates `release-please-config.json`, `.release-please-manifest.json`, and documentation.
   - `taplo fmt --check` validates all `Cargo.toml` manifests.
2. **Packaging Verification:**
   - Workspace packaging validates via `cargo package --workspace --no-verify`.
3. **Nix Flake & Unified Quality Gate:**
   - `nix flake check && nix build` verifies `flake.nix` builds `code-review` cleanly with synchronized `Cargo.lock`.
   - `cargo run -p code-review -- check` and `cargo test --all-targets --all-features` pass with zero warnings or errors.
