use super::file_utils::{find_files_with_extensions, is_tool_available, normalize_path};
use crate::diagnostics::Span;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_edit::DocumentMut;

/// Diagnostics emitted by [`TomlRunner`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, miette::Diagnostic)]
pub enum TomlDiagnostic {
    /// TOML file does not match Taplo formatting rules.
    #[error("TOML formatting in '{file}' does not conform to taplo standards")]
    #[diagnostic(
        code(fmt::toml),
        severity(Warning),
        help("Run 'taplo fmt {file}' to format")
    )]
    Formatting {
        file: String,
        #[source_code]
        #[label]
        span: Span,
    },

    /// TOML syntax error reported by Taplo or `toml_edit`.
    #[error("TOML syntax error in '{file}': {details}")]
    #[diagnostic(code(toml::syntax), severity(Error))]
    SyntaxError {
        file: String,
        details: String,
        #[source_code]
        #[label]
        span: Span,
    },
}

impl TomlDiagnostic {
    /// Returns the source [`Span`] associated with this diagnostic.
    pub const fn span(&self) -> &Span {
        match self {
            Self::Formatting { span, .. } | Self::SyntaxError { span, .. } => span,
        }
    }
}

/// Runner for TOML checks and formatters (Taplo or in-process toml_edit).
pub struct TomlRunner {
    target_path: PathBuf,
}

impl TomlRunner {
    /// Creates a new `TomlRunner` targeting the specified directory or file.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes TOML checks using `taplo` if available, or in-process syntax validation.
    pub fn run(&self) -> Result<Vec<TomlDiagnostic>, io::Error> {
        let files = find_files_with_extensions(&self.target_path, &["toml"]);
        if files.is_empty() {
            return Ok(Vec::new());
        }

        if is_tool_available("taplo") {
            self.run_taplo(&files)
        } else {
            self.run_in_process_syntax_check(&files)
        }
    }

    fn run_taplo(&self, files: &[PathBuf]) -> Result<Vec<TomlDiagnostic>, io::Error> {
        let mut cmd = Command::new("taplo");
        cmd.arg("fmt").arg("--check");
        for file in files {
            cmd.arg(file);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        Ok(parse_taplo_output(&combined, &self.target_path))
    }

    fn run_in_process_syntax_check(
        &self,
        files: &[PathBuf],
    ) -> Result<Vec<TomlDiagnostic>, io::Error> {
        let mut diagnostics = Vec::new();

        for file in files {
            let Ok(content) = fs::read_to_string(file) else {
                continue;
            };

            if let Err(err) = content.parse::<DocumentMut>() {
                let normalized = normalize_path(file, &self.target_path);
                let span = Span::new(normalized, 1, 1, 1, 1);
                diagnostics.push(TomlDiagnostic::SyntaxError {
                    file: file.display().to_string(),
                    details: err.to_string(),
                    span,
                });
            }
        }

        Ok(diagnostics)
    }
}

/// Parses the output of `taplo fmt --check` or `taplo check`.
pub fn parse_taplo_output(output: &str, base_dir: &Path) -> Vec<TomlDiagnostic> {
    let mut diagnostics = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // Taplo fmt output: "error: file not formatted: /path/to/file.toml"
        if let Some(rest) = trimmed.strip_prefix("error: file not formatted: ") {
            let file_str = rest.trim();
            let path = PathBuf::from(file_str);
            let normalized = normalize_path(&path, base_dir);
            let span = Span::new(normalized, 1, 1, 1, 1);
            diagnostics.push(TomlDiagnostic::Formatting {
                file: file_str.to_string(),
                span,
            });
        } else if let Some(rest) = trimmed.strip_prefix("error: ")
            && let Some((file_str, msg)) = rest.split_once(':')
            && file_str.ends_with(".toml")
        {
            let file_trimmed = file_str.trim();
            let path = PathBuf::from(file_trimmed);
            let normalized = normalize_path(&path, base_dir);
            let span = Span::new(normalized, 1, 1, 1, 1);
            diagnostics.push(TomlDiagnostic::SyntaxError {
                file: file_trimmed.to_string(),
                details: msg.trim().to_string(),
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
    fn parse_taplo_output_parses_unformatted_and_syntax_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let sample = "\
error: file not formatted: Cargo.toml
error: invalid.toml: unexpected symbol at line 5
";
        let base = Path::new(".");
        let diags = parse_taplo_output(sample, base);
        expect_that!(diags.len(), eq(2));
        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(d0.code().map(|c| c.to_string()), some(eq("fmt::toml")));
        expect_that!(d0.severity(), some(eq(Severity::Warning)));
        let d1 = diags.get(1).ok_or("missing diag 1")?;
        expect_that!(d1.code().map(|c| c.to_string()), some(eq("toml::syntax")));
        expect_that!(d1.severity(), some(eq(Severity::Error)));
        Ok(())
    }

    #[googletest::test]
    fn in_process_toml_check_flags_syntax_error() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        fs::write(
            temp_dir.path().join("valid.toml"),
            "[package]\nname = \"demo\"\n",
        )?;
        fs::write(
            temp_dir.path().join("invalid.toml"),
            "[package\nname = demo\n",
        )?;

        let runner = TomlRunner::new(temp_dir.path());
        let files = vec![
            temp_dir.path().join("valid.toml"),
            temp_dir.path().join("invalid.toml"),
        ];
        let diags = runner.run_in_process_syntax_check(&files)?;

        expect_that!(diags.len(), eq(1));
        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(d0.code().map(|c| c.to_string()), some(eq("toml::syntax")));
        expect_that!(d0.severity(), some(eq(Severity::Error)));
        Ok(())
    }
}
