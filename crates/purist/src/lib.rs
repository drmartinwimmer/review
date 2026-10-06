pub mod cargo;
pub mod diagnostics;
pub mod engine;
pub mod reporter;
pub mod rules;

pub use cargo::{LintConfig, OpinionatedLintsConfig, PuristLintsConfig, RuleLevel};
pub use diagnostics::{Diagnostic, DiagnosticReport, ReportSummary, Severity, Span};
pub use engine::{LintContext, OpinionatedEngine, PuristEngine, Rule};
pub use reporter::{OutputFormat, render_report, render_report_with_options};
pub use rules::default_rules;
use std::path::{Path, PathBuf};

/// Error type for purist linter execution.
#[derive(Debug, thiserror::Error)]
pub enum PuristError {
    #[error("Target path '{0}' was not found")]
    PathNotFound(PathBuf),

    #[error("I/O error during purist lint execution: {0}")]
    Io(#[from] std::io::Error),

    #[error("Purist lint violations found ({count} issues)")]
    LintViolationsFound { count: usize },
}

/// Backwards compatibility alias for `PuristError`.
pub type OpinionatedError = PuristError;

/// Execution options for running purist linter checks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PuristOptions {
    /// Path to source files or crate directory.
    pub path: Option<PathBuf>,

    /// Output format for reports and diagnostics.
    pub format: Option<OutputFormat>,

    /// Automatically apply fixes where supported (stub).
    pub fix: bool,

    /// Silence non-essential logging output.
    pub quiet: bool,
}

/// Backwards compatibility alias for `PuristOptions`.
pub type OpinionatedOptions = PuristOptions;

impl PuristOptions {
    /// Creates a new `PuristOptions` instance.
    pub fn new(path: Option<PathBuf>, quiet: bool) -> Self {
        Self {
            path,
            format: None,
            fix: false,
            quiet,
        }
    }
}

/// Executes the purist rules against the target path and returns the report.
pub fn execute(options: &PuristOptions) -> Result<DiagnosticReport, PuristError> {
    let target_path = options.path.as_deref().unwrap_or_else(|| Path::new("."));

    if !target_path.exists() {
        return Err(PuristError::PathNotFound(target_path.to_path_buf()));
    }

    let engine = PuristEngine::new();
    let report = engine.check_path(target_path)?;
    Ok(report)
}

/// Runs the purist static analysis checks and renders diagnostics.
pub fn run(options: &PuristOptions) -> Result<(), PuristError> {
    let format = options.format.unwrap_or(OutputFormat::Console);
    let report = execute(options)?;

    render_report(&report, format, &mut std::io::stdout())?;

    if !report.is_empty() {
        Err(PuristError::LintViolationsFound {
            count: report.diagnostics.len(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::fs;

    #[googletest::test]
    fn run_purist_command_on_missing_path_returns_error() -> Result<(), Box<dyn std::error::Error>>
    {
        let missing = PathBuf::from("does_not_exist_12345.rs");
        let opts = PuristOptions::new(Some(missing.clone()), true);
        match execute(&opts) {
            Err(PuristError::PathNotFound(p)) => {
                assert_that!(p, eq(&missing));
            }
            other => return Err(format!("Expected PathNotFound, got {other:?}").into()),
        }
        Ok(())
    }

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    #[googletest::test]
    fn run_purist_command_on_clean_file_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_clean_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("clean.rs");
        fs::write(&file_path, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n")?;

        let opts = PuristOptions::new(Some(file_path), true);
        let report = execute(&opts)?;

        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn run_purist_command_detects_violations() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_violations_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("bad.rs");
        fs::write(
            &file_path,
            "pub fn fail() -> Result<(), String> { Err(\"bad\".to_string()) }\n",
        )?;

        let opts = PuristOptions::new(Some(file_path), true);
        let result = run(&opts);

        match result {
            Err(PuristError::LintViolationsFound { count }) => {
                assert_that!(count, eq(1));
            }
            other => return Err(format!("Expected LintViolationsFound, got {other:?}").into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn parse_purist_command_with_options() -> Result<(), Box<dyn std::error::Error>> {
        let mut opts = PuristOptions::new(Some(PathBuf::from("src")), false);
        opts.format = Some(OutputFormat::Json);
        opts.fix = true;

        assert_that!(&opts.path, eq(&Some(PathBuf::from("src"))));
        assert_that!(opts.format, eq(Some(OutputFormat::Json)));
        assert_that!(opts.fix, is_true());
        assert_that!(opts.quiet, is_false());
        Ok(())
    }
}
