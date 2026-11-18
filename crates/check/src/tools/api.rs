use crate::diagnostics::{Diagnostic, Severity, Span, span_labels};
use api_manifest::{
    ApiManifest, ApiManifestError, Diagnostic as RawApiDiagnostic, Severity as ApiSeverity,
};
use miette::{LabeledSpan, SourceCode};
use std::fmt;
use std::path::{Path, PathBuf};

/// Error type for API manifest verification.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Error from `api-manifest` inspection or manifest checking.
    #[error("API manifest check error: {0}")]
    Manifest(#[from] ApiManifestError),
}

/// Diagnostic emitted by [`ApiRunner`] when public API drift is detected against `API.md`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ApiDiagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub suggested_fix: Option<String>,
}

impl ApiDiagnostic {
    /// Returns the source [`Span`] associated with this diagnostic, if any.
    pub const fn span(&self) -> Option<&Span> {
        self.span.as_ref()
    }
}

impl Diagnostic for ApiDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(&self.code))
    }

    fn severity(&self) -> Option<Severity> {
        Some(self.severity)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.suggested_fix
            .as_deref()
            .map(|fix| Box::new(fix) as Box<dyn fmt::Display>)
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.span().map(|s| s as &dyn SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.span().map(span_labels)
    }
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
    pub fn run(&self) -> Result<Vec<ApiDiagnostic>, ApiError> {
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

fn convert_diagnostic(diag: RawApiDiagnostic) -> ApiDiagnostic {
    let severity = match diag.severity {
        ApiSeverity::Error => Severity::Error,
        ApiSeverity::Warning => Severity::Warning,
        ApiSeverity::Info | ApiSeverity::Hint => Severity::Advice,
    };

    let span = diag
        .span
        .map(|s| Span::new(s.file, s.start_line, s.start_col, s.end_line, s.end_col));

    ApiDiagnostic {
        code: diag.rule,
        severity,
        message: diag.message,
        span,
        suggested_fix: diag.suggested_fix,
    }
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
        assert_that!(diag.severity(), some(eq(Severity::Error)));
        assert_that!(
            diag.code().map(|c| c.to_string()),
            some(eq("api::drift::removal"))
        );

        Ok(())
    }
}
