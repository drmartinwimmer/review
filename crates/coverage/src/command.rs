use crate::parser::CoverageReport;
use crate::tools::LlvmCovRunner;
use clap::Args;
use clapback::CommandError;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Error type for code coverage measurement and threshold enforcement.
#[derive(Debug, thiserror::Error)]
pub enum CoverageError {
    /// Target directory path was not found.
    #[error("Target path '{0}' was not found")]
    PathNotFound(PathBuf),

    /// I/O error during coverage execution or output writing.
    #[error("I/O error during coverage analysis: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing or serialization error.
    #[error("Failed to parse or serialize coverage JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// `cargo llvm-cov` command execution error.
    #[error("cargo llvm-cov execution failed: {0}")]
    Command(#[from] CommandError),

    /// Measured line coverage was strictly below the configured threshold.
    #[error("Coverage {actual:.2}% is below minimum threshold {threshold:.2}%")]
    ThresholdNotMet {
        /// Actual measured line coverage percentage.
        actual: f64,
        /// Required minimum line coverage percentage.
        threshold: f64,
    },
}

/// Arguments for the code coverage measurement and threshold enforcement subcommand.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct CoverageCommand {
    /// Path to target workspace or crate directory
    #[arg(long)]
    path: Option<PathBuf>,

    /// Minimum line coverage threshold percentage (0.0 to 100.0)
    #[arg(long, alias = "threshold")]
    fail_under: Option<f64>,

    /// Emit coverage report as structured JSON
    #[arg(long)]
    json: bool,

    /// Write LCOV-formatted coverage summary to file
    #[arg(long)]
    lcov: Option<PathBuf>,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl CoverageCommand {
    fn evaluate_report(
        &self,
        report: &CoverageReport,
        writer: &mut dyn Write,
    ) -> Result<(), CoverageError> {
        if let Some(lcov_path) = &self.lcov {
            if let Some(parent) = lcov_path.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)?;
            }
            fs::write(lcov_path, report.format_lcov())?;
        }

        if !self.quiet {
            if self.json {
                report.render_json(writer)?;
                writeln!(writer)?;
            } else {
                report.render_console(writer)?;
            }
        }

        if let Some(threshold) = self.fail_under {
            let actual = report.line_coverage_percent();
            if actual < threshold {
                return Err(CoverageError::ThresholdNotMet { actual, threshold });
            }
        }

        Ok(())
    }

    /// Runs `cargo llvm-cov` and enforces any configured coverage threshold.
    pub fn run(self) -> Result<(), CoverageError> {
        let target_dir = self.path.clone().unwrap_or_else(|| PathBuf::from("."));
        let runner = LlvmCovRunner::new(target_dir);
        let report = runner.run()?;
        self.evaluate_report(&report, &mut std::io::stdout())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_llvm_cov_json;
    use clap::Parser;
    use googletest::prelude::*;
    use std::path::Path;

    const SAMPLE_JSON: &str = r#"{
      "data": [
        {
          "totals": {
            "lines": { "count": 100, "covered": 85, "percent": 85.0 },
            "functions": { "count": 20, "covered": 18, "percent": 90.0 },
            "regions": { "count": 150, "covered": 120, "percent": 80.0 }
          },
          "files": []
        }
      ]
    }"#;

    #[derive(Parser, Debug)]
    struct TestCli {
        #[command(flatten)]
        cmd: CoverageCommand,
    }

    #[googletest::test]
    fn parse_coverage_command_flags_populates_fields() -> Result<(), Box<dyn std::error::Error>> {
        let cli = TestCli::try_parse_from([
            "coverage",
            "--path",
            "crates/check",
            "--fail-under",
            "85.5",
            "--json",
            "--lcov",
            "lcov.info",
            "--quiet",
        ])?;
        expect_that!(cli.cmd.path.as_deref(), eq(Some(Path::new("crates/check"))));
        expect_that!(cli.cmd.fail_under, eq(Some(85.5)));
        expect_that!(cli.cmd.json, is_true());
        expect_that!(cli.cmd.lcov.as_deref(), eq(Some(Path::new("lcov.info"))));
        expect_that!(cli.cmd.quiet, is_true());
        Ok(())
    }

    #[googletest::test]
    fn evaluate_report_passes_when_above_threshold() -> Result<(), Box<dyn std::error::Error>> {
        let report = parse_llvm_cov_json(SAMPLE_JSON)?;
        let cmd = CoverageCommand {
            fail_under: Some(80.0),
            ..Default::default()
        };
        let mut out = Vec::new();
        assert_that!(cmd.evaluate_report(&report, &mut out), ok(anything()));
        expect_that!(out.is_empty(), is_false());
        Ok(())
    }

    #[googletest::test]
    fn evaluate_report_fails_when_below_threshold() -> Result<(), Box<dyn std::error::Error>> {
        let report = parse_llvm_cov_json(SAMPLE_JSON)?;
        let cmd = CoverageCommand {
            fail_under: Some(90.0),
            quiet: true,
            ..Default::default()
        };
        let mut out = Vec::new();
        match cmd.evaluate_report(&report, &mut out) {
            Err(CoverageError::ThresholdNotMet { actual, threshold }) => {
                expect_that!(actual, eq(85.0));
                expect_that!(threshold, eq(90.0));
            }
            other => return Err(format!("Expected ThresholdNotMet, got {other:?}").into()),
        }
        Ok(())
    }
}
