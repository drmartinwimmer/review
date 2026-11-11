pub use crate::presets::LintProfile;
use crate::presets::{LINT_CATEGORIES, LintCategory, RUST_LINTS};
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml_edit::{Item, Table, TomlError, Value, value};

/// Result metadata returned after modifying or inspecting a Cargo.toml file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigureResult {
    /// True if the file contents were modified on disk; false if already up-to-date or no lints removed.
    pub modified: bool,
    /// For `configure_lints`: The number of lints configured in the targeted table.
    /// For `remove_lints`: The number of lints removed from the targeted table (or 0 if none found).
    pub lints_configured: usize,
}

/// Errors that can occur during Cargo.toml inspection or modification.
#[derive(Debug, Error)]
pub enum CargoTomlError {
    #[error("Manifest file not found at path: {path}")]
    ManifestNotFound { path: PathBuf },

    #[error("I/O error accessing manifest file: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("Failed to parse TOML manifest: {source}")]
    Parse {
        #[from]
        source: TomlError,
    },

    /// Returned when an expected table (such as `[workspace]`, `[package]`, `[lints]`,
    /// `[lints.rust]`, or `[lints.clippy]`) exists in the document but is a scalar or
    /// array rather than a table or inline table.
    #[error("Invalid Cargo.toml structure: '{table}' is not a table")]
    NotATable { table: &'static str },

    /// Returned when `[package.lints]` is defined instead of root-level `[lints]`.
    #[error("Invalid Cargo.toml structure: 'lints' must be at root level, not under [package]")]
    LintsUnderPackage,

    /// Returned when the manifest contains neither `[workspace]` nor `[package]`.
    #[error("Invalid Cargo.toml structure: Manifest contains neither [workspace] nor [package]")]
    MissingWorkspaceAndPackage,
}

#[derive(Clone, Copy)]
enum LintsScope {
    Workspace,
    Crate,
}

impl LintsScope {
    const fn rust_table(self) -> &'static str {
        match self {
            Self::Workspace => "workspace.lints.rust",
            Self::Crate => "lints.rust",
        }
    }

    const fn clippy_table(self) -> &'static str {
        match self {
            Self::Workspace => "workspace.lints.clippy",
            Self::Crate => "lints.clippy",
        }
    }
}

fn validate_lints_subtables(
    lints: &dyn toml_edit::TableLike,
    scope: LintsScope,
) -> Result<(), CargoTomlError> {
    for (subtable, table_path) in [
        ("rust", scope.rust_table()),
        ("clippy", scope.clippy_table()),
    ] {
        if let Some(item) = lints.get(subtable)
            && item.as_table_like().is_none()
        {
            return Err(CargoTomlError::NotATable { table: table_path });
        }
    }
    Ok(())
}

fn validate_document_structure(doc: &toml_edit::DocumentMut) -> Result<(), CargoTomlError> {
    if let Some(item) = doc.get("workspace") {
        let ws = item
            .as_table_like()
            .ok_or(CargoTomlError::NotATable { table: "workspace" })?;
        if let Some(lints_item) = ws.get("lints") {
            let lints = lints_item
                .as_table_like()
                .ok_or(CargoTomlError::NotATable {
                    table: "workspace.lints",
                })?;
            validate_lints_subtables(lints, LintsScope::Workspace)?;
        }
    }

    if let Some(item) = doc.get("package") {
        let pkg = item
            .as_table_like()
            .ok_or(CargoTomlError::NotATable { table: "package" })?;
        if pkg.get("lints").is_some() {
            return Err(CargoTomlError::LintsUnderPackage);
        }
    }

    if let Some(item) = doc.get("lints") {
        let lints = item
            .as_table_like()
            .ok_or(CargoTomlError::NotATable { table: "lints" })?;
        validate_lints_subtables(lints, LintsScope::Crate)?;
    }

    Ok(())
}

fn format_category_comment(category: &LintCategory, is_first_category: bool) -> String {
    let leading_newline = if is_first_category { "" } else { "\n" };
    if category.description.is_empty() {
        format!("{leading_newline}# {}\n", category.title)
    } else {
        format!(
            "{leading_newline}# {} - {}\n",
            category.title, category.description
        )
    }
}

fn populate_empty_clippy_table(table: &mut toml_edit::Table, profile: LintProfile) {
    let mut is_first_category = true;
    for category in LINT_CATEGORIES {
        let active_lints: Vec<&'static str> = category
            .lints
            .iter()
            .copied()
            .filter(|lint| profile.includes_lint(lint))
            .collect();

        if active_lints.is_empty() {
            continue;
        }

        let comment = format_category_comment(category, is_first_category);

        for (idx, lint) in active_lints.into_iter().enumerate() {
            table.insert(lint, toml_edit::value("warn"));
            if idx == 0
                && let Some(mut k) = table.key_mut(lint)
            {
                k.leaf_decor_mut().set_prefix(comment.as_str());
            }
        }

        is_first_category = false;
    }
}

fn extract_matching_key_prefix(
    table: &toml_edit::Table,
    key: &str,
    expected_header: &str,
) -> Option<String> {
    let k = table.key(key)?;
    let prefix = k.leaf_decor().prefix()?.as_str()?;
    prefix.contains(expected_header).then(|| prefix.to_string())
}

fn restore_transferred_prefix(
    table: &mut toml_edit::Table,
    key: &str,
    expected_header: &str,
    comment: Option<String>,
) {
    if let Some(comment) = comment
        && let Some(mut k) = table.key_mut(key)
        && k.leaf_decor()
            .prefix()
            .and_then(|p| p.as_str())
            .is_none_or(|s| !s.contains(expected_header))
    {
        k.leaf_decor_mut().set_prefix(comment);
    }
}

fn update_existing_clippy_table(table: &mut toml_edit::Table, profile: LintProfile) {
    let mut transferred_panic_comment = None;
    let mut transferred_allow_comment = None;

    if profile == LintProfile::Standard {
        transferred_panic_comment =
            extract_matching_key_prefix(table, "string_slice", "# Don't Panic");
        transferred_allow_comment =
            extract_matching_key_prefix(table, "allow_attributes", "# Don't `allow`");

        table.remove("string_slice");
        table.remove("indexing_slicing");
        table.remove("allow_attributes");
    }

    for category in LINT_CATEGORIES {
        for lint in category.lints {
            if profile.includes_lint(lint) {
                set_lint_to_warn(table, lint);
            }
        }
    }

    restore_transferred_prefix(
        table,
        "unwrap_used",
        "# Don't Panic",
        transferred_panic_comment,
    );
    restore_transferred_prefix(
        table,
        "allow_attributes_without_reason",
        "# Don't `allow`",
        transferred_allow_comment,
    );
}

fn populate_rust_table(table: &mut toml_edit::Table, profile: LintProfile) {
    for lint in RUST_LINTS {
        if profile.includes_lint(lint) {
            set_lint_to_warn(table, lint);
        }
    }
}

fn populate_clippy_table(table: &mut toml_edit::Table, profile: LintProfile) {
    if table.is_empty() {
        populate_empty_clippy_table(table, profile);
    } else {
        update_existing_clippy_table(table, profile);
    }
}

fn set_lint_to_warn(table: &mut Table, lint: &str) {
    if let Some(existing) = table.get_mut(lint) {
        if let Some(v) = existing.as_value_mut()
            && v.as_str() != Some("warn")
        {
            *existing = toml_edit::value("warn");
        }
    } else {
        table.insert(lint, toml_edit::value("warn"));
    }
}

fn populate_lints_subtables(
    lints_table: &mut Table,
    scope: LintsScope,
    profile: LintProfile,
) -> Result<(), CargoTomlError> {
    if !lints_table.contains_key("rust") {
        let mut rust_table = Table::new();
        rust_table.set_implicit(false);
        lints_table.insert("rust", Item::Table(rust_table));
    }

    let rust_table = lints_table
        .get_mut("rust")
        .and_then(|i| i.as_table_mut())
        .ok_or(CargoTomlError::NotATable {
            table: scope.rust_table(),
        })?;

    populate_rust_table(rust_table, profile);

    if !lints_table.contains_key("clippy") {
        let mut clippy_table = Table::new();
        clippy_table.set_implicit(false);
        lints_table.insert("clippy", Item::Table(clippy_table));
    }

    let clippy_table = lints_table
        .get_mut("clippy")
        .and_then(|i| i.as_table_mut())
        .ok_or(CargoTomlError::NotATable {
            table: scope.clippy_table(),
        })?;

    populate_clippy_table(clippy_table, profile);

    Ok(())
}

fn configure_workspace_lints(
    doc: &mut toml_edit::DocumentMut,
    profile: LintProfile,
    has_package: bool,
) -> Result<(), CargoTomlError> {
    let ws = doc
        .get_mut("workspace")
        .and_then(|i| i.as_table_mut())
        .ok_or(CargoTomlError::NotATable { table: "workspace" })?;

    if !ws.contains_key("lints") {
        let mut lints_table = Table::new();
        lints_table.set_implicit(true);
        ws.insert("lints", Item::Table(lints_table));
    }

    let ws_lints =
        ws.get_mut("lints")
            .and_then(|i| i.as_table_mut())
            .ok_or(CargoTomlError::NotATable {
                table: "workspace.lints",
            })?;

    populate_lints_subtables(ws_lints, LintsScope::Workspace, profile)?;

    if has_package {
        ensure_root_lints_workspace_inheritance(doc)?;
    }

    Ok(())
}

fn ensure_root_lints_workspace_inheritance(
    doc: &mut toml_edit::DocumentMut,
) -> Result<(), CargoTomlError> {
    if let Some(lints_item) = doc.get_mut("lints") {
        let lints_table = lints_item
            .as_table_mut()
            .ok_or(CargoTomlError::NotATable { table: "lints" })?;
        let is_ws_true = lints_table
            .get("workspace")
            .and_then(|v| v.as_value())
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !is_ws_true {
            lints_table.insert("workspace", value(true));
        }
    } else {
        let mut lints_table = Table::new();
        lints_table.insert("workspace", value(true));
        doc.insert("lints", Item::Table(lints_table));
    }
    Ok(())
}

fn configure_crate_lints(
    doc: &mut toml_edit::DocumentMut,
    profile: LintProfile,
) -> Result<(), CargoTomlError> {
    if !doc.contains_key("lints") {
        let mut lints_table = Table::new();
        lints_table.set_implicit(true);
        doc.insert("lints", Item::Table(lints_table));
    }

    let lints_table = doc
        .get_mut("lints")
        .and_then(|i| i.as_table_mut())
        .ok_or(CargoTomlError::NotATable { table: "lints" })?;

    populate_lints_subtables(lints_table, LintsScope::Crate, profile)
}

/// Configures strict Clippy lints in the specified Cargo.toml manifest.
pub fn configure_lints(
    manifest_path: &Path,
    profile: LintProfile,
) -> Result<ConfigureResult, CargoTomlError> {
    if !manifest_path.exists() {
        return Err(CargoTomlError::ManifestNotFound {
            path: manifest_path.to_path_buf(),
        });
    }

    let original_raw = std::fs::read_to_string(manifest_path)?;
    let mut doc: toml_edit::DocumentMut = original_raw.parse()?;

    validate_document_structure(&doc)?;

    let has_workspace = doc.contains_key("workspace");
    let has_package = doc.contains_key("package");

    if !has_workspace && !has_package {
        return Err(CargoTomlError::MissingWorkspaceAndPackage);
    }

    if has_workspace {
        configure_workspace_lints(&mut doc, profile, has_package)?;
    } else {
        configure_crate_lints(&mut doc, profile)?;
    }

    let new_raw = doc.to_string();
    let modified = new_raw != original_raw;

    if modified {
        std::fs::write(manifest_path, &new_raw)?;
    }

    Ok(ConfigureResult {
        modified,
        lints_configured: profile.expected_count(),
    })
}

fn remove_configured_rust_lints(lints_table: &mut Table) -> usize {
    let mut removed = 0;
    let should_remove_rust_table = if let Some(rust_item) = lints_table.get_mut("rust")
        && let Some(rust_table) = rust_item.as_table_like_mut()
    {
        for lint in RUST_LINTS {
            if rust_table.remove(lint).is_some() {
                removed += 1;
            }
        }
        rust_table.is_empty()
    } else {
        false
    };

    if should_remove_rust_table {
        lints_table.remove("rust");
    }

    removed
}

fn remove_from_lints_table(lints_table: &mut Table) -> usize {
    let mut removed_count = remove_configured_rust_lints(lints_table);
    if let Some(clippy_item) = lints_table.remove("clippy")
        && let Some(clippy_table) = clippy_item.as_table_like()
    {
        removed_count += clippy_table.iter().count();
    }
    removed_count
}

/// Removes configured Clippy lints and associated tables from the specified Cargo.toml manifest.
pub fn remove_lints(manifest_path: &Path) -> Result<ConfigureResult, CargoTomlError> {
    if !manifest_path.exists() {
        return Err(CargoTomlError::ManifestNotFound {
            path: manifest_path.to_path_buf(),
        });
    }

    let original_raw = std::fs::read_to_string(manifest_path)?;
    let mut doc: toml_edit::DocumentMut = original_raw.parse()?;

    validate_document_structure(&doc)?;

    let mut removed_count = 0;

    // 1. Check [lints]
    if let Some(lints_item) = doc.get_mut("lints")
        && let Some(lints_table) = lints_item.as_table_mut()
    {
        removed_count += remove_from_lints_table(lints_table);
        if lints_table.is_empty() {
            doc.remove("lints");
        }
    }

    // 2. Check [workspace.lints]
    if let Some(ws_item) = doc.get_mut("workspace")
        && let Some(ws_table) = ws_item.as_table_mut()
        && let Some(ws_lints_item) = ws_table.get_mut("lints")
        && let Some(ws_lints_table) = ws_lints_item.as_table_mut()
    {
        removed_count += remove_from_lints_table(ws_lints_table);
        if ws_lints_table.is_empty() {
            ws_table.remove("lints");
        }
    }

    // 3. In Case 2 (Root package with workspace):
    // If [lints] only has `workspace = true` and [workspace.lints] was pruned, prune [lints]
    let ws_has_lints = doc
        .get("workspace")
        .and_then(|w| w.as_table_like())
        .is_some_and(|w| w.contains_key("lints"));

    if !ws_has_lints
        && let Some(lints_item) = doc.get_mut("lints")
        && let Some(lints_table) = lints_item.as_table_mut()
        && lints_table.len() == 1
        && lints_table.contains_key("workspace")
    {
        doc.remove("lints");
    }

    let new_raw = doc.to_string();
    let modified = new_raw != original_raw;

    if modified {
        std::fs::write(manifest_path, &new_raw)?;
    }

    Ok(ConfigureResult {
        modified,
        lints_configured: removed_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::fs;
    use tempfile::TempDir;

    struct TempManifest {
        path: PathBuf,
        _dir: TempDir,
    }

    impl TempManifest {
        fn new(content: &str) -> Result<Self, Box<dyn std::error::Error>> {
            let dir = tempfile::tempdir()?;
            let path = dir.path().join("Cargo.toml");
            fs::write(&path, content)?;
            Ok(Self { path, _dir: dir })
        }

        fn read(&self) -> Result<String, Box<dyn std::error::Error>> {
            Ok(fs::read_to_string(&self.path)?)
        }
    }

    #[googletest::test]
    fn configure_lints_with_comments_and_whitespace_preserves_formatting()
    -> Result<(), Box<dyn std::error::Error>> {
        let original = r#"# Top-level comment
[package]
name = "demo" # inline comment
version = "0.1.0"

# Dependency section comment
[dependencies]
serde = "1.0" # inline dep comment

# Trailing comment at end
"#;
        let manifest = TempManifest::new(original)?;
        let result = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(result.modified, is_true());
        expect_that!(result.lints_configured, eq(51));

        let modified_content = manifest.read()?;
        expect_that!(modified_content, contains_substring("# Top-level comment"));
        expect_that!(
            modified_content,
            contains_substring("name = \"demo\" # inline comment")
        );
        expect_that!(
            modified_content,
            contains_substring("# Dependency section comment")
        );
        expect_that!(
            modified_content,
            contains_substring("serde = \"1.0\" # inline dep comment")
        );
        expect_that!(
            modified_content,
            contains_substring("# Trailing comment at end")
        );
        expect_that!(modified_content, contains_substring("[lints.rust]"));
        expect_that!(
            modified_content,
            contains_substring("missing_copy_implementations = \"warn\"")
        );
        expect_that!(modified_content, contains_substring("[lints.clippy]"));
        expect_that!(
            modified_content,
            contains_substring("unwrap_used = \"warn\"")
        );
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_twice_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"
"#;
        let manifest = TempManifest::new(initial)?;
        let res1 = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res1.modified, is_true());
        expect_that!(res1.lints_configured, eq(51));

        let content_after_run1 = manifest.read()?;

        let res2 = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res2.modified, is_false());
        expect_that!(res2.lints_configured, eq(51));

        let content_after_run2 = manifest.read()?;
        expect_that!(content_after_run1, eq(&content_after_run2));
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_virtual_workspace_injects_only_workspace_clippy()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[workspace]
members = ["crates/*"]
"#;
        let manifest = TempManifest::new(initial)?;
        let res = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res.modified, is_true());
        expect_that!(res.lints_configured, eq(51));

        let content = manifest.read()?;
        expect_that!(content, contains_substring("[workspace.lints.rust]"));
        expect_that!(content, contains_substring("[workspace.lints.clippy]"));
        expect_that!(content, not(contains_substring("\n[lints]")));
        expect_that!(content, not(contains_substring("workspace = true")));
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_root_package_with_workspace_injects_workspace_clippy_and_workspace_true()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "root"
version = "0.1.0"

[workspace]
members = ["crates/*"]
"#;
        let manifest = TempManifest::new(initial)?;
        let res = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res.modified, is_true());
        expect_that!(res.lints_configured, eq(51));

        let content = manifest.read()?;
        expect_that!(content, contains_substring("[workspace.lints.rust]"));
        expect_that!(content, contains_substring("[workspace.lints.clippy]"));
        expect_that!(content, contains_substring("workspace = true"));
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_single_crate_injects_lints_clippy() -> Result<(), Box<dyn std::error::Error>>
    {
        let initial = r#"[package]
name = "single"
version = "0.1.0"
"#;
        let manifest = TempManifest::new(initial)?;
        let res = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res.modified, is_true());
        expect_that!(res.lints_configured, eq(51));

        let content = manifest.read()?;
        expect_that!(content, contains_substring("[lints.rust]"));
        expect_that!(content, contains_substring("[lints.clippy]"));
        expect_that!(content, not(contains_substring("[workspace")));
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_profile_differences_strict_and_standard()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"
"#;
        let manifest_strict = TempManifest::new(initial)?;
        let res_strict = configure_lints(&manifest_strict.path, LintProfile::Strict)?;
        expect_that!(res_strict.lints_configured, eq(51));
        let content_strict = manifest_strict.read()?;
        for expected in [
            "missing_copy_implementations = \"warn\"",
            "string_slice = \"warn\"",
            "indexing_slicing = \"warn\"",
            "allow_attributes = \"warn\"",
            "cognitive_complexity = \"warn\"",
            "too_many_lines = \"warn\"",
            "cast_possible_truncation = \"warn\"",
            "cast_possible_wrap = \"warn\"",
            "cast_precision_loss = \"warn\"",
            "checked_conversions = \"warn\"",
            "ignored_unit_patterns = \"warn\"",
            "large_stack_arrays = \"warn\"",
            "exit = \"warn\"",
            "infinite_loop = \"warn\"",
            "large_types_passed_by_value = \"warn\"",
            "manual_let_else = \"warn\"",
            "match_same_arms = \"warn\"",
            "redundant_closure_for_method_calls = \"warn\"",
            "map_unwrap_or = \"warn\"",
            "redundant_clone = \"warn\"",
        ] {
            expect_that!(content_strict, contains_substring(expected));
        }

        let manifest_standard = TempManifest::new(initial)?;
        let res_standard = configure_lints(&manifest_standard.path, LintProfile::Standard)?;
        expect_that!(res_standard.lints_configured, eq(48));
        let content_standard = manifest_standard.read()?;
        expect_that!(
            content_standard,
            not(contains_substring("string_slice = \"warn\""))
        );
        expect_that!(
            content_standard,
            not(contains_substring("indexing_slicing = \"warn\""))
        );
        expect_that!(
            content_standard,
            not(contains_substring("allow_attributes = \"warn\""))
        );
        expect_that!(
            content_standard,
            contains_substring("allow_attributes_without_reason = \"warn\"")
        );
        expect_that!(
            content_standard,
            contains_substring("unwrap_used = \"warn\"")
        );
        expect_that!(
            content_standard,
            contains_substring("missing_copy_implementations = \"warn\"")
        );
        expect_that!(
            content_standard,
            contains_substring("redundant_clone = \"warn\"")
        );
        Ok(())
    }

    #[googletest::test]
    fn remove_lints_cleans_clippy_and_prunes_empty_parent_table()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"
"#;
        let manifest = TempManifest::new(initial)?;
        configure_lints(&manifest.path, LintProfile::Strict)?;

        let remove_res = remove_lints(&manifest.path)?;
        expect_that!(remove_res.modified, is_true());
        expect_that!(remove_res.lints_configured, eq(51));

        let content_after_remove = manifest.read()?;
        expect_that!(content_after_remove, not(contains_substring("[lints]")));
        expect_that!(
            content_after_remove,
            not(contains_substring("[lints.rust]"))
        );
        expect_that!(
            content_after_remove,
            not(contains_substring("[lints.clippy]"))
        );

        let remove_again = remove_lints(&manifest.path)?;
        expect_that!(remove_again.modified, is_false());
        expect_that!(remove_again.lints_configured, eq(0));
        Ok(())
    }

    #[googletest::test]
    fn remove_lints_with_existing_rust_lints_preserves_parent_table()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"

[lints.rust]
unsafe_code = "forbid"
"#;
        let manifest = TempManifest::new(initial)?;
        configure_lints(&manifest.path, LintProfile::Strict)?;

        let content_configured = manifest.read()?;
        expect_that!(content_configured, contains_substring("[lints.rust]"));
        expect_that!(content_configured, contains_substring("[lints.clippy]"));

        let remove_res = remove_lints(&manifest.path)?;
        expect_that!(remove_res.modified, is_true());
        expect_that!(remove_res.lints_configured, eq(51));

        let content_after_remove = manifest.read()?;
        expect_that!(content_after_remove, contains_substring("[lints.rust]"));
        expect_that!(
            content_after_remove,
            contains_substring("unsafe_code = \"forbid\"")
        );
        expect_that!(
            content_after_remove,
            not(contains_substring("missing_copy_implementations"))
        );
        expect_that!(
            content_after_remove,
            not(contains_substring("[lints.clippy]"))
        );
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_missing_file_returns_manifest_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        let missing = PathBuf::from("/non/existent/path/Cargo.toml");
        match configure_lints(&missing, LintProfile::Strict) {
            Err(CargoTomlError::ManifestNotFound { path }) => {
                expect_that!(path, eq(&missing));
            }
            other => return Err(format!("Expected ManifestNotFound, got {other:?}").into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_malformed_toml_returns_parse_error() -> Result<(), Box<dyn std::error::Error>>
    {
        let malformed = "this is not valid toml = [[";
        let manifest = TempManifest::new(malformed)?;
        match configure_lints(&manifest.path, LintProfile::Strict) {
            Err(CargoTomlError::Parse { .. }) => Ok(()),
            other => Err(format!("Expected Parse error, got {other:?}").into()),
        }
    }

    #[googletest::test]
    fn configure_lints_scalar_lints_or_workspace_returns_invalid_structure()
    -> Result<(), Box<dyn std::error::Error>> {
        let invalid_lints = r#"lints = "invalid_scalar"

[package]
name = "demo"
version = "0.1.0"
"#;
        let manifest1 = TempManifest::new(invalid_lints)?;
        match configure_lints(&manifest1.path, LintProfile::Strict) {
            Err(CargoTomlError::NotATable { table: "lints" }) => {}
            other => {
                return Err(format!("Expected NotATable for scalar lints, got {other:?}").into());
            }
        }

        let invalid_ws = r#"workspace = 42
"#;
        let manifest2 = TempManifest::new(invalid_ws)?;
        match configure_lints(&manifest2.path, LintProfile::Strict) {
            Err(CargoTomlError::NotATable { table: "workspace" }) => {}
            other => {
                return Err(
                    format!("Expected NotATable for scalar workspace, got {other:?}").into(),
                );
            }
        }
        Ok(())
    }

    #[googletest::test]
    fn remove_lints_with_empty_clippy_table_prunes_empty_tables_on_disk()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"

[lints.clippy]
"#;
        let manifest = TempManifest::new(initial)?;
        let remove_res = remove_lints(&manifest.path)?;
        expect_that!(remove_res.modified, is_true());
        expect_that!(remove_res.lints_configured, eq(0));

        let content_after = manifest.read()?;
        expect_that!(content_after, not(contains_substring("[lints]")));
        expect_that!(content_after, not(contains_substring("[lints.clippy]")));
        Ok(())
    }

    #[googletest::test]
    fn remove_lints_virtual_workspace_prunes_workspace_lints()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[workspace]
members = ["crates/*"]
"#;
        let manifest = TempManifest::new(initial)?;
        configure_lints(&manifest.path, LintProfile::Strict)?;
        let content_configured = manifest.read()?;
        expect_that!(
            content_configured,
            contains_substring("[workspace.lints.clippy]")
        );

        let remove_res = remove_lints(&manifest.path)?;
        expect_that!(remove_res.modified, is_true());
        expect_that!(remove_res.lints_configured, eq(51));

        let content_after = manifest.read()?;
        expect_that!(content_after, not(contains_substring("[workspace.lints]")));
        expect_that!(content_after, not(contains_substring("clippy")));
        expect_that!(content_after, contains_substring("[workspace]"));
        expect_that!(
            content_after,
            contains_substring("members = [\"crates/*\"]")
        );
        Ok(())
    }

    #[googletest::test]
    fn remove_lints_root_package_with_workspace_prunes_workspace_lints_and_root_lints()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "root"
version = "0.1.0"

[workspace]
members = ["crates/*"]
"#;
        let manifest = TempManifest::new(initial)?;
        configure_lints(&manifest.path, LintProfile::Strict)?;
        let content_configured = manifest.read()?;
        expect_that!(
            content_configured,
            contains_substring("[workspace.lints.clippy]")
        );
        expect_that!(content_configured, contains_substring("[lints]"));

        let remove_res = remove_lints(&manifest.path)?;
        expect_that!(remove_res.modified, is_true());
        expect_that!(remove_res.lints_configured, eq(51));

        let content_after = manifest.read()?;
        expect_that!(content_after, not(contains_substring("[workspace.lints]")));
        expect_that!(content_after, not(contains_substring("[lints]")));
        expect_that!(content_after, contains_substring("[package]"));
        expect_that!(content_after, contains_substring("[workspace]"));
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_downgrade_to_standard_preserves_category_comments()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"
"#;
        let manifest = TempManifest::new(initial)?;
        let res_strict = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res_strict.modified, is_true());
        let content_strict = manifest.read()?;
        expect_that!(content_strict, contains_substring("# Don't Panic"));
        expect_that!(content_strict, contains_substring("# Don't `allow`"));

        let res_standard = configure_lints(&manifest.path, LintProfile::Standard)?;
        expect_that!(res_standard.modified, is_true());
        expect_that!(res_standard.lints_configured, eq(48));

        let content_standard = manifest.read()?;
        expect_that!(content_standard, not(contains_substring("string_slice")));
        expect_that!(
            content_standard,
            not(contains_substring("allow_attributes ="))
        );
        expect_that!(content_standard, contains_substring("# Don't Panic"));
        expect_that!(content_standard, contains_substring("# Don't `allow`"));
        expect_that!(
            content_standard,
            contains_substring("unwrap_used = \"warn\"")
        );
        expect_that!(
            content_standard,
            contains_substring("allow_attributes_without_reason = \"warn\"")
        );
        Ok(())
    }
}
