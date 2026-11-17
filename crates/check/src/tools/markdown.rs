use super::file_utils::{find_files_with_extensions, is_tool_available, normalize_path};
use crate::diagnostics::Span;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Diagnostics emitted by [`MarkdownRunner`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, miette::Diagnostic)]
pub enum MarkdownDiagnostic {
    /// Markdown file does not match Prettier formatting rules.
    #[error("Markdown formatting in '{file}' does not conform to prettier standards")]
    #[diagnostic(
        code(fmt::markdown),
        severity(Warning),
        help("Run 'prettier --write {file}' to format")
    )]
    PrettierFormatting {
        file: String,
        #[source_code]
        #[label]
        span: Span,
    },

    /// Markdown file does not match mdformat formatting rules.
    #[error("Markdown formatting in '{file}' does not conform to mdformat standards")]
    #[diagnostic(
        code(fmt::markdown),
        severity(Warning),
        help("Run 'mdformat {file}' to format")
    )]
    MdformatFormatting {
        file: String,
        #[source_code]
        #[label]
        span: Span,
    },

    /// Markdown syntax error reported by Prettier.
    #[error("Syntax error in '{file}': {details}")]
    #[diagnostic(code(fmt::markdown), severity(Error))]
    SyntaxError {
        file: String,
        details: String,
        #[source_code]
        #[label]
        span: Span,
    },
}

impl MarkdownDiagnostic {
    /// Returns the source [`Span`] associated with this diagnostic.
    pub const fn span(&self) -> &Span {
        match self {
            Self::PrettierFormatting { span, .. }
            | Self::MdformatFormatting { span, .. }
            | Self::SyntaxError { span, .. } => span,
        }
    }
}

/// Runner for Markdown checks and formatters (Prettier / mdformat).
pub struct MarkdownRunner {
    target_path: PathBuf,
}

impl MarkdownRunner {
    /// Creates a new `MarkdownRunner` targeting the specified directory or file.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes markdown checks using the first available tool (`prettier` or `mdformat`).
    pub fn run(&self) -> Result<Vec<MarkdownDiagnostic>, io::Error> {
        let files = find_files_with_extensions(&self.target_path, &["md"]);
        if files.is_empty() {
            return Ok(Vec::new());
        }

        if is_tool_available("prettier") {
            self.run_prettier(&files)
        } else if is_tool_available("mdformat") {
            self.run_mdformat(&files)
        } else {
            Ok(Vec::new())
        }
    }

    fn run_prettier(&self, files: &[PathBuf]) -> Result<Vec<MarkdownDiagnostic>, io::Error> {
        let mut cmd = Command::new("prettier");
        cmd.arg("--check");
        for file in files {
            cmd.arg(file);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        Ok(parse_prettier_markdown_output(&combined, &self.target_path))
    }

    fn run_mdformat(&self, files: &[PathBuf]) -> Result<Vec<MarkdownDiagnostic>, io::Error> {
        let mut cmd = Command::new("mdformat");
        cmd.arg("--check");
        for file in files {
            cmd.arg(file);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        Ok(parse_mdformat_output(&combined, &self.target_path))
    }
}

/// Parses the output of `prettier --check` on Markdown files.
pub fn parse_prettier_markdown_output(output: &str, base_dir: &Path) -> Vec<MarkdownDiagnostic> {
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
            diagnostics.push(MarkdownDiagnostic::PrettierFormatting {
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
            diagnostics.push(MarkdownDiagnostic::SyntaxError {
                file: file_str.to_string(),
                details: err_msg.trim().to_string(),
                span,
            });
        }
    }

    diagnostics
}

/// Parses the output of `mdformat --check`.
pub fn parse_mdformat_output(output: &str, base_dir: &Path) -> Vec<MarkdownDiagnostic> {
    let mut diagnostics = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        // Format: Error: File "/path/to/file.md" is not formatted.
        if let Some(rest) = trimmed.strip_prefix("Error: File \"")
            && let Some((file_str, _)) = rest.split_once("\" is not formatted")
        {
            let path = PathBuf::from(file_str);
            let normalized = normalize_path(&path, base_dir);
            let span = Span::new(normalized, 1, 1, 1, 1);
            diagnostics.push(MarkdownDiagnostic::MdformatFormatting {
                file: file_str.to_string(),
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

    #[googletest::test]
    fn parse_prettier_markdown_output_parses_warnings_and_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let sample = "\
Checking formatting...
AGENTS.md[warn] AGENTS.md
[warn] /workspace/SPEC.md
[warn] Code style issues found in 2 files. Run Prettier with --write to fix.
[error] /workspace/broken.md: SyntaxError: Unexpected token
";
        let base = Path::new("/workspace");
        let diags = parse_prettier_markdown_output(sample, base);
        expect_that!(diags.len(), eq(3));

        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(d0.code().map(|c| c.to_string()), some(eq("fmt::markdown")));
        expect_that!(d0.severity(), some(eq(Severity::Warning)));
        expect_that!(d0.span().file, eq(Path::new("AGENTS.md")));

        let d1 = diags.get(1).ok_or("missing diag 1")?;
        expect_that!(d1.code().map(|c| c.to_string()), some(eq("fmt::markdown")));
        expect_that!(d1.severity(), some(eq(Severity::Warning)));
        expect_that!(d1.span().file, eq(Path::new("SPEC.md")));

        let d2 = diags.get(2).ok_or("missing diag 2")?;
        expect_that!(d2.code().map(|c| c.to_string()), some(eq("fmt::markdown")));
        expect_that!(d2.severity(), some(eq(Severity::Error)));
        expect_that!(d2.span().file, eq(Path::new("broken.md")));
        Ok(())
    }

    #[googletest::test]
    fn parse_mdformat_output_parses_unformatted_files() -> Result<(), Box<dyn std::error::Error>> {
        let sample = "\
Error: File \"/workspace/README.md\" is not formatted.
Error: File \"/workspace/AGENTS.md\" is not formatted.
";
        let base = Path::new("/workspace");
        let diags = parse_mdformat_output(sample, base);
        expect_that!(diags.len(), eq(2));

        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(d0.code().map(|c| c.to_string()), some(eq("fmt::markdown")));
        expect_that!(d0.span().file, eq(Path::new("README.md")));

        let d1 = diags.get(1).ok_or("missing diag 1")?;
        expect_that!(d1.code().map(|c| c.to_string()), some(eq("fmt::markdown")));
        expect_that!(d1.span().file, eq(Path::new("AGENTS.md")));
        Ok(())
    }
}
