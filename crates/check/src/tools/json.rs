use super::file_utils::{find_files_with_extensions, is_tool_available, normalize_path};
use crate::diagnostics::Span;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Diagnostics emitted by [`JsonRunner`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, miette::Diagnostic)]
pub enum JsonDiagnostic {
    /// JSON file does not match Prettier formatting rules.
    #[error("JSON formatting in '{file}' does not conform to prettier standards")]
    #[diagnostic(
        code(fmt::json),
        severity(Warning),
        help("Run 'prettier --write {file}' to format")
    )]
    Formatting {
        file: String,
        #[source_code]
        #[label]
        span: Span,
    },

    /// JSON syntax error reported by Prettier or `serde_json`.
    #[error("JSON syntax error in '{file}': {details}")]
    #[diagnostic(code(json::syntax), severity(Error))]
    SyntaxError {
        file: String,
        details: String,
        #[source_code]
        #[label]
        span: Span,
    },
}

impl JsonDiagnostic {
    /// Returns the source [`Span`] associated with this diagnostic.
    pub const fn span(&self) -> &Span {
        match self {
            Self::Formatting { span, .. } | Self::SyntaxError { span, .. } => span,
        }
    }
}

/// Runner for JSON checks and formatters (Prettier or in-process serde_json).
pub struct JsonRunner {
    target_path: PathBuf,
}

impl JsonRunner {
    /// Creates a new `JsonRunner` targeting the specified directory or file.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes JSON checks using `prettier` if available, or in-process syntax validation.
    pub fn run(&self) -> Result<Vec<JsonDiagnostic>, io::Error> {
        let files = find_files_with_extensions(&self.target_path, &["json"]);
        if files.is_empty() {
            return Ok(Vec::new());
        }

        if is_tool_available("prettier") {
            self.run_prettier(&files)
        } else {
            self.run_in_process_syntax_check(&files)
        }
    }

    fn run_prettier(&self, files: &[PathBuf]) -> Result<Vec<JsonDiagnostic>, io::Error> {
        let mut cmd = Command::new("prettier");
        cmd.arg("--check");
        for file in files {
            cmd.arg(file);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        Ok(parse_prettier_json_output(&combined, &self.target_path))
    }

    fn run_in_process_syntax_check(
        &self,
        files: &[PathBuf],
    ) -> Result<Vec<JsonDiagnostic>, io::Error> {
        let mut diagnostics = Vec::new();

        for file in files {
            let Ok(content) = fs::read_to_string(file) else {
                continue;
            };

            if let Err(err) = serde_json::from_str::<serde_json::Value>(&content) {
                let line = err.line();
                let col = err.column();
                let normalized = normalize_path(file, &self.target_path);
                let span = Span::new(normalized, line, col, line, col);
                diagnostics.push(JsonDiagnostic::SyntaxError {
                    file: file.display().to_string(),
                    details: err.to_string(),
                    span,
                });
            }
        }

        Ok(diagnostics)
    }
}

/// Parses the output of `prettier --check` on JSON files.
pub fn parse_prettier_json_output(output: &str, base_dir: &Path) -> Vec<JsonDiagnostic> {
    let mut diagnostics = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();

        if let Some((_, rest)) = trimmed.split_once("[warn] ") {
            if rest.starts_with("Code style issues found") {
                continue;
            }
            let file_str = rest.trim();
            if file_str.is_empty() {
                continue;
            }

            let path = PathBuf::from(file_str);
            let normalized = normalize_path(&path, base_dir);
            let span = Span::new(normalized, 1, 1, 1, 1);
            diagnostics.push(JsonDiagnostic::Formatting {
                file: file_str.to_string(),
                span,
            });
        } else if let Some((_, rest)) = trimmed.split_once("[error] ")
            && let Some((file_part, err_msg)) = rest.split_once(':')
        {
            let file_str = file_part.trim();
            let path = PathBuf::from(file_str);
            let normalized = normalize_path(&path, base_dir);
            let span = Span::new(normalized, 1, 1, 1, 1);
            diagnostics.push(JsonDiagnostic::SyntaxError {
                file: file_str.to_string(),
                details: err_msg.trim().to_string(),
                span,
            });
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{Diagnostic, Severity};
    use googletest::prelude::*;
    use tempfile::tempdir;

    #[googletest::test]
    fn parse_prettier_json_output_parses_warnings_and_syntax_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let sample = "\
Checking formatting...
[warn] package.json
[warn] Code style issues found in 1 file. Run Prettier with --write to fix.
[error] bad.json: SyntaxError: Unexpected token
";
        let base = Path::new(".");
        let diags = parse_prettier_json_output(sample, base);
        expect_that!(diags.len(), eq(2));
        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(d0.code().map(|c| c.to_string()), some(eq("fmt::json")));
        expect_that!(d0.severity(), some(eq(Severity::Warning)));
        let d1 = diags.get(1).ok_or("missing diag 1")?;
        expect_that!(d1.code().map(|c| c.to_string()), some(eq("json::syntax")));
        expect_that!(d1.severity(), some(eq(Severity::Error)));
        Ok(())
    }

    #[googletest::test]
    fn in_process_syntax_check_flags_syntax_error() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        fs::write(temp_dir.path().join("valid.json"), "{\"name\": \"test\"}")?;
        fs::write(temp_dir.path().join("invalid.json"), "{ invalid: json }")?;

        let runner = JsonRunner::new(temp_dir.path());
        let files = vec![
            temp_dir.path().join("valid.json"),
            temp_dir.path().join("invalid.json"),
        ];
        let diags = runner.run_in_process_syntax_check(&files)?;

        expect_that!(diags.len(), eq(1));
        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(d0.code().map(|c| c.to_string()), some(eq("json::syntax")));
        expect_that!(d0.severity(), some(eq(Severity::Error)));
        Ok(())
    }
}
