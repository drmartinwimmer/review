use crate::tools::{
    AuditRunner, ClippyRunner, FmtRunner, JjError, JjVcs, JsonRunner, MarkdownRunner, PuristRunner,
    TomlRunner, aggregate_diagnostics, filter_diagnostics_by_changed_files,
};
use purist::{DiagnosticReport, OutputFormat, render_report};
use std::path::{Path, PathBuf};

/// Severity threshold triggering non-zero exit code.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    clap::ValueEnum,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum FailOn {
    /// Fail if any warnings or errors are found.
    #[default]
    Warnings,
    /// Fail only if errors are found.
    Errors,
}

/// Error type for check aggregator execution.
#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("Target path '{0}' was not found")]
    PathNotFound(PathBuf),

    #[error("Jujutsu VCS error: {0}")]
    Vcs(#[from] JjError),

    #[error("I/O error during check execution: {0}")]
    Io(#[from] std::io::Error),

    #[error("Check violations found ({count} issues exceed failure threshold)")]
    ViolationsFound { count: usize },
}

/// Execution options for running check aggregator checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOptions {
    /// Path to target workspace or crate directory.
    pub path: Option<PathBuf>,

    /// Output format for reports and diagnostics.
    pub format: Option<OutputFormat>,

    /// Severity threshold triggering non-zero exit code.
    pub fail_on: FailOn,

    /// Filter diagnostics to only files modified in Jujutsu working copy.
    pub changed_only: bool,

    /// Run cargo fmt checks (enabled by default).
    pub fmt: bool,

    /// Run cargo clippy checks (enabled by default).
    pub clippy: bool,

    /// Run purist AST linter checks (enabled by default).
    pub purist: bool,

    /// Run cargo audit dependency security scan (enabled by default).
    pub audit: bool,

    /// Run markdown format/lint checks (enabled by default).
    pub markdown: bool,

    /// Run TOML format/lint checks (enabled by default).
    pub toml: bool,

    /// Run JSON format/lint checks (enabled by default).
    pub json: bool,

    /// Silence non-essential logging output.
    pub quiet: bool,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            path: None,
            format: None,
            fail_on: FailOn::Warnings,
            changed_only: false,
            fmt: true,
            clippy: true,
            purist: true,
            audit: true,
            markdown: true,
            toml: true,
            json: true,
            quiet: false,
        }
    }
}

impl CheckOptions {
    /// Creates a new `CheckOptions` instance with default options.
    pub fn new(path: Option<PathBuf>, quiet: bool) -> Self {
        Self {
            path,
            quiet,
            ..Default::default()
        }
    }

    /// Executes all configured checking tools and returns the aggregated diagnostic report.
    pub fn execute(&self) -> Result<DiagnosticReport, CheckError> {
        execute(self)
    }

    /// Runs the check aggregator, rendering reports and returning violation errors.
    pub fn run(&self) -> Result<(), CheckError> {
        run(self)
    }
}

/// Executes all configured checking tools and returns the aggregated diagnostic report.
pub fn execute(options: &CheckOptions) -> Result<DiagnosticReport, CheckError> {
    let target_dir = options.path.as_deref().unwrap_or_else(|| Path::new("."));

    if !target_dir.exists() {
        return Err(CheckError::PathNotFound(target_dir.to_path_buf()));
    }

    let fmt_diags = if !options.fmt {
        Vec::new()
    } else {
        let runner = FmtRunner::new(target_dir);
        runner.run()?
    };

    let clippy_diags = if !options.clippy {
        Vec::new()
    } else {
        let runner = ClippyRunner::new(target_dir);
        runner.run()?
    };

    let purist_report = if !options.purist {
        DiagnosticReport::default()
    } else {
        let runner = PuristRunner::new(target_dir);
        runner.run()?
    };

    let audit_diags = if !options.audit {
        Vec::new()
    } else {
        let runner = AuditRunner::new(target_dir);
        runner.run()?
    };

    let markdown_diags = if !options.markdown {
        Vec::new()
    } else {
        let runner = MarkdownRunner::new(target_dir);
        runner.run()?
    };

    let toml_diags = if !options.toml {
        Vec::new()
    } else {
        let runner = TomlRunner::new(target_dir);
        runner.run()?
    };

    let json_diags = if !options.json {
        Vec::new()
    } else {
        let runner = JsonRunner::new(target_dir);
        runner.run()?
    };

    let aggregated = aggregate_diagnostics(
        fmt_diags,
        clippy_diags,
        purist_report,
        audit_diags,
        markdown_diags,
        toml_diags,
        json_diags,
    );

    if options.changed_only {
        let vcs = JjVcs::new(target_dir);
        let changed_files = vcs.query_changed_files()?;
        Ok(filter_diagnostics_by_changed_files(
            aggregated,
            &changed_files,
            target_dir,
        ))
    } else {
        Ok(aggregated)
    }
}

/// Runs the check aggregator, rendering reports and returning violation errors.
pub fn run(options: &CheckOptions) -> Result<(), CheckError> {
    let format = options.format.unwrap_or(OutputFormat::Console);
    let fail_on = options.fail_on;
    let report = execute(options)?;

    render_report(&report, format, &mut std::io::stdout())?;

    let fails = match fail_on {
        FailOn::Warnings => report.has_errors() || report.warning_count() > 0,
        FailOn::Errors => report.has_errors(),
    };

    if fails {
        let count = match fail_on {
            FailOn::Warnings => report.error_count() + report.warning_count(),
            FailOn::Errors => report.error_count(),
        };
        Err(CheckError::ViolationsFound { count })
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
    fn parse_check_command_with_flags_populates_fields() -> Result<(), Box<dyn std::error::Error>> {
        let mut opts = CheckOptions::new(Some(PathBuf::from("crates/check")), false);
        opts.format = Some(OutputFormat::Json);
        opts.fail_on = FailOn::Errors;
        opts.changed_only = true;
        opts.fmt = false;
        opts.clippy = false;
        opts.purist = false;
        opts.audit = false;
        opts.markdown = false;
        opts.toml = false;
        opts.json = false;

        assert_that!(&opts.path, eq(&Some(PathBuf::from("crates/check"))));
        assert_that!(opts.format, eq(Some(OutputFormat::Json)));
        assert_that!(opts.fail_on, eq(FailOn::Errors));
        assert_that!(opts.changed_only, is_true());
        assert_that!(opts.fmt, is_false());
        assert_that!(opts.clippy, is_false());
        assert_that!(opts.purist, is_false());
        assert_that!(opts.audit, is_false());
        assert_that!(opts.markdown, is_false());
        assert_that!(opts.toml, is_false());
        assert_that!(opts.json, is_false());
        assert_that!(opts.quiet, is_false());
        Ok(())
    }

    #[googletest::test]
    fn execute_on_missing_path_returns_path_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let missing = PathBuf::from("non_existent_dir_99999");
        let opts = CheckOptions::new(Some(missing.clone()), true);

        match opts.execute() {
            Err(CheckError::PathNotFound(p)) => {
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
    fn execute_with_all_checks_skipped_returns_empty_report()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_check_all_skipped_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        let mut opts = CheckOptions::new(Some(temp_dir), true);
        opts.fmt = false;
        opts.clippy = false;
        opts.purist = false;
        opts.audit = false;
        opts.markdown = false;
        opts.toml = false;
        opts.json = false;

        let report = opts.execute()?;
        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn run_with_skipped_checks_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_check_run_ok_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        let mut opts = CheckOptions::new(Some(temp_dir), true);
        opts.fmt = false;
        opts.clippy = false;
        opts.purist = false;
        opts.audit = false;
        opts.markdown = false;
        opts.toml = false;
        opts.json = false;

        assert_that!(opts.run(), ok(anything()));
        Ok(())
    }

    #[googletest::test]
    fn run_with_violation_and_fail_on_warnings_fails() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_check_viol_warn_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let bad_file = temp_dir.join("lib.rs");
        fs::write(
            &bad_file,
            "pub fn fail() -> Result<(), String> { Err(\"bad\".to_string()) }\n",
        )?;

        let mut opts = CheckOptions::new(Some(temp_dir), true);
        opts.fmt = false;
        opts.clippy = false;
        opts.audit = false;
        opts.markdown = false;
        opts.toml = false;
        opts.json = false;
        opts.fail_on = FailOn::Warnings;

        match opts.run() {
            Err(CheckError::ViolationsFound { count }) => {
                assert_that!(count, eq(1));
            }
            other => return Err(format!("Expected ViolationsFound, got {other:?}").into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn run_with_warning_and_fail_on_errors_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_check_viol_err_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let bad_file = temp_dir.join("lib.rs");
        fs::write(
            &bad_file,
            "pub fn fail() -> Result<(), String> { Err(\"bad\".to_string()) }\n",
        )?;

        let mut opts = CheckOptions::new(Some(temp_dir), true);
        opts.fmt = false;
        opts.clippy = false;
        opts.audit = false;
        opts.markdown = false;
        opts.toml = false;
        opts.json = false;
        opts.fail_on = FailOn::Errors;

        assert_that!(opts.run(), ok(anything()));
        Ok(())
    }
}
