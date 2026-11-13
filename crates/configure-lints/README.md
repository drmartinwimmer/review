# code-review-configure-lints

> **Programmatic `Cargo.toml` Clippy lint configurator preserving comments and formatting.**

`code-review-configure-lints` injects or removes curated, strict Clippy and Rust lint configurations in standalone crate or Cargo workspace `Cargo.toml` manifests using `toml_edit` without stripping comments or disturbing existing table layout.

## Configured Lint Groups

1. **Don't Panic**: `unwrap_used`, `indexing_slicing`, `string_slice`, `panic`, `todo`, `unimplemented`, `panic_in_result_fn`
2. **Don't Fail Silently**: `let_underscore_future`, `let_underscore_must_use`, `unused_result_ok`, `map_err_ignore`
3. **Memory Safety**: `mem_forget`, `undocumented_unsafe_blocks`, `multiple_unsafe_ops_per_block`, `large_stack_arrays`
4. **Numeric Precision & Casts**: `float_cmp`, `lossy_float_literal`, `cast_sign_loss`, `cast_possible_truncation`, `checked_conversions`
5. **Suppression Hygiene**: `allow_attributes`, `allow_attributes_without_reason`
6. **Maintainability & Complexity**: `cognitive_complexity`, `too_many_lines`, `redundant_clone`, `missing_copy_implementations`

## Installation

```bash
cargo install code-review-configure-lints --bin configure-lints
```

## Usage

```bash
# Inject strict Clippy lints into Cargo.toml
configure-lints

# Use standard preset profile
configure-lints --profile standard

# Remove configured lints cleanly
configure-lints --remove
```
