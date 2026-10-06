use code_review_api::{diff_manifests, extract_crate_api, read_from_file};
use code_review_diagnostics::{Diagnostic, Severity, Span};
use std::fs;
use std::path::{Path, PathBuf};

/// Runner for public API manifest drift verification (`API.md`).
pub struct ApiRunner {
    target_path: PathBuf,
}

impl ApiRunner {
    /// Creates a new `ApiRunner` targeting the specified directory or workspace.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes API drift verification against all found `API.md` manifests.
    pub fn run(&self) -> Result<Vec<Diagnostic>, std::io::Error> {
        let root = if self.target_path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            self.target_path.as_path()
        };

        let manifests = find_api_manifests(root);
        let mut all_diags = Vec::new();

        for manifest_path in manifests {
            let crate_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
            let baseline = match read_from_file(&manifest_path) {
                Ok(b) => b,
                Err(err) => {
                    all_diags.push(
                        Diagnostic::new(
                            "api::manifest_parse_error",
                            Severity::Error,
                            format!("Failed to parse API manifest '{manifest_path:?}': {err}"),
                        )
                        .with_span(Span::new(
                            manifest_path.display().to_string(),
                            1,
                            1,
                            1,
                            1,
                        )),
                    );
                    continue;
                }
            };

            let active = match extract_crate_api(crate_dir) {
                Ok(a) => a,
                Err(err) => {
                    all_diags.push(
                        Diagnostic::new(
                            "api::extraction_error",
                            Severity::Error,
                            format!("Failed to extract API for crate at '{crate_dir:?}': {err}"),
                        )
                        .with_span(Span::new(
                            manifest_path.display().to_string(),
                            1,
                            1,
                            1,
                            1,
                        )),
                    );
                    continue;
                }
            };

            let report = diff_manifests(&active, &baseline);
            if !report.is_clean() {
                let diags = report.to_diagnostics(&manifest_path.display().to_string());
                all_diags.extend(diags);
            }
        }

        Ok(all_diags)
    }
}

/// Discovers `API.md` files in the target root or subdirectories.
fn find_api_manifests(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        if root.file_name().and_then(|s| s.to_str()) == Some("API.md") {
            return vec![root.to_path_buf()];
        }
        return Vec::new();
    }

    let mut manifests = Vec::new();
    crawl_api_manifests(root, &mut manifests);
    manifests.sort();
    manifests
}

fn crawl_api_manifests(dir: &Path, manifests: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };

        if path.is_dir() {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            crawl_api_manifests(&path, manifests);
        } else if path.is_file() && name == "API.md" {
            manifests.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use code_review_api::format_markdown;
    use googletest::prelude::*;

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    #[googletest::test]
    fn run_when_no_manifest_returns_empty_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_api_runner_empty_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        let runner = ApiRunner::new(&temp_dir);
        let diags = runner.run()?;
        assert_that!(diags.is_empty(), is_true());

        Ok(())
    }

    #[googletest::test]
    fn run_when_manifest_clean_returns_no_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_api_runner_clean_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        let cargo_toml = r#"
            [package]
            name = "clean-crate"
            version = "0.1.0"
        "#;
        fs::write(temp_dir.join("Cargo.toml"), cargo_toml)?;
        let src_dir = temp_dir.join("src");
        fs::create_dir_all(&src_dir)?;
        fs::write(src_dir.join("lib.rs"), "pub fn hello() {}\n")?;

        let manifest = extract_crate_api(&temp_dir)?;
        fs::write(temp_dir.join("API.md"), format_markdown(&manifest))?;

        let runner = ApiRunner::new(&temp_dir);
        let diags = runner.run()?;
        assert_that!(diags.is_empty(), is_true());

        Ok(())
    }

    #[googletest::test]
    fn run_when_manifest_has_drift_returns_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_api_runner_drift_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        let cargo_toml = r#"
            [package]
            name = "drift-crate"
            version = "0.1.0"
        "#;
        fs::write(temp_dir.join("Cargo.toml"), cargo_toml)?;
        let src_dir = temp_dir.join("src");
        fs::create_dir_all(&src_dir)?;
        fs::write(src_dir.join("lib.rs"), "pub fn original() {}\n")?;

        let modified_manifest = r#"# Public API Manifest: drift-crate (v0.1.0)

## 1. Library API

### Functions

- `pub fn original()`
"#;
        fs::write(temp_dir.join("API.md"), modified_manifest)?;

        let runner = ApiRunner::new(&temp_dir);
        let diags = runner.run()?;
        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("diag missing")?;
        assert_that!(diag.severity, eq(Severity::Error));
        assert_that!(diag.rule, eq("api::drift::removal"));

        Ok(())
    }
}
