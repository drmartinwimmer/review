use purist::{PuristDiagnostic, PuristEngine};
use std::io;
use std::path::{Path, PathBuf};

/// Runner for purist AST static analysis rules.
pub struct PuristRunner {
    target_path: PathBuf,
}

impl PuristRunner {
    /// Creates a new `PuristRunner` targeting the specified directory or file.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes purist rules against the target path and returns all emitted diagnostics.
    pub fn run(&self) -> Result<Vec<PuristDiagnostic>, io::Error> {
        let path = if self.target_path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            self.target_path.as_path()
        };

        let engine = PuristEngine::new();
        Ok(engine.check_path(path)?.diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Diagnostic;
    use googletest::prelude::*;
    use std::fs;
    use tempfile::tempdir;

    #[googletest::test]
    fn run_purist_runner_on_clean_code_produces_no_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"clean\"\nversion = \"0.1.0\"\n",
        )?;
        let file_path = temp_dir.path().join("lib.rs");
        fs::write(&file_path, "pub fn calculate(a: i32) -> i32 { a * 2 }\n")?;

        let runner = PuristRunner::new(file_path);
        let diags = runner.run()?;
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn run_purist_runner_on_violation_produces_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"viol\"\nversion = \"0.1.0\"\n",
        )?;
        let file_path = temp_dir.path().join("lib.rs");
        fs::write(
            &file_path,
            "pub fn broken() -> Result<(), String> { Err(\"error\".into()) }\n",
        )?;

        let runner = PuristRunner::new(file_path);
        let diags = runner.run()?;
        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            diag.code().map(|c| c.to_string()),
            some(eq("purist::error_types"))
        );
        Ok(())
    }
}
