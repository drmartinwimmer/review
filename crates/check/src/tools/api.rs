use api_manifest::{
    ApiManifest, ApiManifestError, Diagnostic as ApiDiagnostic, Severity as ApiSeverity,
};
use purist::{Diagnostic, Severity, Span};
use std::path::{Path, PathBuf};

/// Error type for API manifest verification.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Error from `api-manifest` inspection or manifest checking.
    #[error("API manifest check error: {0}")]
    Manifest(#[from] ApiManifestError),
}

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
    pub fn run(&self) -> Result<Vec<Diagnostic>, ApiError> {
        let root = if self.target_path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            self.target_path.as_path()
        };

        let report = ApiManifest::check_path(root)?;

        Ok(report
            .diagnostics
            .into_iter()
            .map(convert_diagnostic)
            .collect())
    }
}

fn convert_diagnostic(diag: ApiDiagnostic) -> Diagnostic {
    let severity = match diag.severity {
        ApiSeverity::Hint => Severity::Hint,
        ApiSeverity::Info => Severity::Info,
        ApiSeverity::Warning => Severity::Warning,
        ApiSeverity::Error => Severity::Error,
    };

    let mut mapped = Diagnostic::new(diag.rule, severity, diag.message);
    if let Some(span) = diag.span {
        mapped = mapped.with_span(Span::new(
            span.file,
            span.start_line,
            span.start_col,
            span.end_line,
            span.end_col,
        ));
    }
    if let Some(fix) = diag.suggested_fix {
        mapped = mapped.with_suggested_fix(fix);
    }
    mapped
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::fs;
    use tempfile::tempdir;

    #[googletest::test]
    fn run_when_no_manifest_returns_empty_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        let runner = ApiRunner::new(temp_dir.path());
        let diags = runner.run()?;
        assert_that!(diags.is_empty(), is_true());

        Ok(())
    }

    #[googletest::test]
    fn run_when_manifest_clean_returns_no_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        let cargo_toml = r#"
            [package]
            name = "clean-crate"
            version = "0.1.0"
        "#;
        fs::write(temp_dir.path().join("Cargo.toml"), cargo_toml)?;
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir)?;
        fs::write(src_dir.join("lib.rs"), "pub fn hello() {}\n")?;

        let manifest = ApiManifest::extract_from_crate(temp_dir.path())?;
        fs::write(temp_dir.path().join("API.md"), manifest.format_markdown())?;

        let runner = ApiRunner::new(temp_dir.path());
        let diags = runner.run()?;
        assert_that!(diags.is_empty(), is_true());

        Ok(())
    }

    #[googletest::test]
    fn run_when_manifest_has_breaking_drift_returns_error_diagnostic()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        let cargo_toml = r#"
            [package]
            name = "drift-crate"
            version = "0.1.0"
        "#;
        fs::write(temp_dir.path().join("Cargo.toml"), cargo_toml)?;
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir)?;
        fs::write(src_dir.join("lib.rs"), "// removed\n")?;

        let modified_manifest = r#"# Public API Manifest: drift-crate (v0.1.0)

## 1. Library API

### Functions

- `pub fn original()`
"#;
        fs::write(temp_dir.path().join("API.md"), modified_manifest)?;

        let runner = ApiRunner::new(temp_dir.path());
        let diags = runner.run()?;
        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("diag missing")?;
        assert_that!(diag.severity, eq(Severity::Error));
        assert_that!(&diag.rule, eq("api::drift::removal"));

        Ok(())
    }
}
