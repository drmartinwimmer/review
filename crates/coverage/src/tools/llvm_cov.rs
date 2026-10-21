use crate::command::CoverageError;
use crate::parser::{CoverageReport, parse_llvm_cov_json};
use clapback::{Command, CommandExt};
use std::path::{Path, PathBuf};

/// Declarative `cargo llvm-cov` CLI invocation built with `clapback`.
#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(
    program = "cargo",
    args = ["llvm-cov", "--all-targets", "--all-features", "--json"]
)]
struct CargoLlvmCovCommand {
    #[command(current_dir)]
    current_dir: PathBuf,
}

/// Subprocess runner driving `cargo llvm-cov --all-targets --all-features --json`.
pub struct LlvmCovRunner {
    target_path: PathBuf,
}

impl LlvmCovRunner {
    /// Creates a new `LlvmCovRunner` targeting the specified crate or workspace directory.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes `cargo llvm-cov` and parses the JSON coverage report.
    pub fn run(&self) -> Result<CoverageReport, CoverageError> {
        let dir = if self.target_path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            self.target_path.as_path()
        };

        if !dir.exists() {
            return Err(CoverageError::PathNotFound(dir.to_path_buf()));
        }

        let cmd = CargoLlvmCovCommand {
            current_dir: dir.to_path_buf(),
        };
        let stdout = cmd.stdout_text()?;
        let report = parse_llvm_cov_json(&stdout)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn cargo_llvm_cov_command_builds_expected_display_and_cwd() {
        let cmd = CargoLlvmCovCommand {
            current_dir: PathBuf::from("/workspace"),
        };
        expect_that!(
            cmd.to_string(),
            eq("cargo llvm-cov --all-targets --all-features --json")
        );
        expect_that!(cmd.current_dir_path(), some(eq(Path::new("/workspace"))));
    }

    #[googletest::test]
    fn run_on_missing_path_returns_path_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let missing = PathBuf::from("nonexistent_coverage_dir_99999");
        let runner = LlvmCovRunner::new(&missing);
        match runner.run() {
            Err(CoverageError::PathNotFound(p)) => {
                expect_that!(p, eq(&missing));
            }
            other => return Err(format!("Expected PathNotFound, got {other:?}").into()),
        }
        Ok(())
    }
}
