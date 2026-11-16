use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Profile defining which preset of Rust, Cargo, and Clippy lints to inject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintProfile {
    /// Comprehensive safety, robustness, memory safety, numerics, suppression hygiene, Cargo manifest, and complexity rules (67 lints).
    #[default]
    Strict,
    /// Standard baseline safety, robustness, and Cargo manifest rules (64 lints), omitting the strictest slicing
    /// and suppression bans (`string_slice`, `indexing_slicing`, and `allow_attributes`).
    Standard,
}

impl LintProfile {
    pub(crate) fn includes_lint(&self, lint: &str) -> bool {
        match self {
            Self::Strict => true,
            Self::Standard => !matches!(
                lint,
                "string_slice" | "indexing_slicing" | "allow_attributes"
            ),
        }
    }

    pub(crate) fn expected_count(&self) -> usize {
        let cargo_count = CARGO_LINTS
            .iter()
            .filter(|lint| self.includes_lint(lint))
            .count();
        let rust_count = RUST_LINTS
            .iter()
            .filter(|lint| self.includes_lint(lint))
            .count();
        let clippy_count = LINT_CATEGORIES
            .iter()
            .flat_map(|category| category.lints.iter())
            .filter(|lint| self.includes_lint(lint))
            .count();
        cargo_count + rust_count + clippy_count
    }
}

pub(crate) const CARGO_LINTS: &[&str] = &[
    "non_kebab_case_bins",
    "non_kebab_case_packages",
    "redundant_homepage",
    "unused_dependencies",
    "unused_workspace_dependencies",
    "unused_workspace_package_fields",
];

pub(crate) const RUST_LINTS: &[&str] = &[
    "ambiguous_negative_literals",
    "deref_into_dyn_supertrait",
    "impl_trait_redundant_captures",
    "macro_use_extern_crate",
    "missing_copy_implementations",
    "non_ascii_idents",
    "redundant_lifetimes",
    "unit_bindings",
    "unsafe_op_in_unsafe_fn",
    "unused_lifetimes",
    "unused_macro_rules",
];

pub(crate) struct LintCategory {
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    pub(crate) lints: &'static [&'static str],
}

pub(crate) const LINT_CATEGORIES: &[LintCategory] = &[
    LintCategory {
        title: "Don't Panic",
        description: "prevent panics from unwraps and unsafe slicing or indexing",
        lints: &[
            "string_slice",
            "indexing_slicing",
            "unwrap_used",
            "panic",
            "todo",
            "unimplemented",
            "unreachable",
            "get_unwrap",
            "unwrap_in_result",
            "unchecked_time_subtraction",
            "panic_in_result_fn",
        ],
    },
    LintCategory {
        title: "Don't Fail Silently",
        description: "prevent dropped futures and swallowed errors",
        lints: &[
            "let_underscore_future",
            "let_underscore_must_use",
            "unused_result_ok",
            "map_err_ignore",
            "assertions_on_result_states",
            "ignored_unit_patterns",
        ],
    },
    LintCategory {
        title: "Don't Do Unsafe Things with Memory",
        description: "",
        lints: &[
            "mem_forget",
            "undocumented_unsafe_blocks",
            "multiple_unsafe_ops_per_block",
            "unnecessary_safety_doc",
            "unnecessary_safety_comment",
            "large_stack_arrays",
        ],
    },
    LintCategory {
        title: "Don't Do Potentially Incorrect Things with Numbers",
        description: "",
        lints: &[
            "float_cmp",
            "float_cmp_const",
            "lossy_float_literal",
            "cast_sign_loss",
            "cast_possible_truncation",
            "cast_possible_wrap",
            "cast_precision_loss",
            "checked_conversions",
            "invalid_upcast_comparisons",
        ],
    },
    LintCategory {
        title: "Don't Do Bad Things That are Easy to Avoid",
        description: "",
        lints: &[
            "rc_mutex",
            "debug_assert_with_mut_call",
            "iter_not_returning_iterator",
            "expl_impl_clone_on_copy",
            "infallible_try_from",
            "dbg_macro",
            "exit",
            "infinite_loop",
            "large_types_passed_by_value",
            "manual_let_else",
            "match_same_arms",
            "redundant_closure_for_method_calls",
            "map_unwrap_or",
            "redundant_clone",
        ],
    },
    LintCategory {
        title: "Don't `allow` Your Way Around These Lints",
        description: "every suppression must be a deliberate #[expect(..., reason = \"…\")] rather than a silent #[allow]",
        lints: &["allow_attributes", "allow_attributes_without_reason"],
    },
    LintCategory {
        title: "Maintainability & Complexity",
        description: "keep functions easy to understand",
        lints: &["cognitive_complexity", "too_many_lines"],
    },
];
