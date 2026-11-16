# code-review-configure-lints

> **Programmatic `Cargo.toml` Rust, Cargo, and Clippy lint configurator preserving comments and formatting.**

`code-review-configure-lints` injects or removes curated, strict Rust, Cargo, and Clippy lint configurations in standalone crate or Cargo workspace `Cargo.toml` manifests using `toml_edit` without stripping comments or disturbing existing table layout.

## Configured Lint Groups

1. **Cargo Manifest & Dependency Hygiene (`[lints.cargo]`)**: `unused_dependencies`, `unused_workspace_dependencies`, `unused_workspace_package_fields`, `redundant_homepage`, `non_kebab_case_bins`, `non_kebab_case_packages`
2. **Rust Compiler Lints (`[lints.rust]`)**: `ambiguous_negative_literals`, `deref_into_dyn_supertrait`, `impl_trait_redundant_captures`, `macro_use_extern_crate`, `missing_copy_implementations`, `non_ascii_idents`, `redundant_lifetimes`, `unit_bindings`, `unsafe_op_in_unsafe_fn`, `unused_lifetimes`, `unused_macro_rules`
3. **Don't Panic**: `unwrap_used`, `indexing_slicing`, `string_slice`, `panic`, `todo`, `unimplemented`, `panic_in_result_fn`
4. **Don't Fail Silently**: `let_underscore_future`, `let_underscore_must_use`, `unused_result_ok`, `map_err_ignore`
5. **Memory Safety**: `mem_forget`, `undocumented_unsafe_blocks`, `multiple_unsafe_ops_per_block`, `large_stack_arrays`
6. **Numeric Precision & Casts**: `float_cmp`, `lossy_float_literal`, `cast_sign_loss`, `cast_possible_truncation`, `checked_conversions`
7. **Suppression Hygiene**: `allow_attributes`, `allow_attributes_without_reason`
8. **Maintainability & Complexity**: `cognitive_complexity`, `too_many_lines`, `redundant_clone`

## Installation

```bash
cargo install code-review-configure-lints --bin configure-lints
```

## Usage

```bash
# Inject strict Rust, Cargo, and Clippy lints into Cargo.toml
configure-lints

# Use standard preset profile
configure-lints --profile standard

# Remove configured lints cleanly
configure-lints --remove
```
