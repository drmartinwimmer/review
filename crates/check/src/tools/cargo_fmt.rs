use super::file_utils::normalize_path;
use crate::diagnostics::Span;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Diagnostics emitted by [`FmtRunner`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, miette::Diagnostic)]
pub enum FmtDiagnostic {
    /// A Rust source file has formatting differences reported by `rustfmt`.
    #[error("Source code formatting in '{file}' does not conform to rustfmt standards")]
    #[diagnostic(
        code(fmt::formatting),
        severity(Warning),
        help("Run 'cargo fmt' to format '{file}'")
    )]
    FormattingDiff {
        file: String,
        #[source_code]
        #[label]
        span: Span,
    },

    /// `cargo fmt` exited with a non-zero status without file-level diff headers.
    #[error("{details}")]
    #[diagnostic(
        code(fmt::formatting),
        severity(Warning),
        help("Run 'cargo fmt' to format the codebase")
    )]
    ExecutionFailure { details: String },
}

impl FmtDiagnostic {
    /// Returns the source [`Span`] associated with this diagnostic, if any.
    pub const fn span(&self) -> Option<&Span> {
        match self {
            Self::FormattingDiff { span, .. } => Some(span),
            Self::ExecutionFailure { .. } => None,
        }
    }
}

/// Runner for `cargo fmt --check`.
pub struct FmtRunner {
    target_path: PathBuf,
}

impl FmtRunner {
    /// Creates a new `FmtRunner` targeting the specified directory or workspace.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes `cargo fmt --check` and returns collected diagnostics.
    pub fn run(&self) -> Result<Vec<FmtDiagnostic>, io::Error> {
        let manifest_path = if self.target_path.is_file() {
            self.target_path.clone()
        } else {
            self.target_path.join("Cargo.toml")
        };

        let mut cmd = Command::new("cargo");
        cmd.arg("fmt").arg("--all").arg("--check");

        if manifest_path.exists() {
            cmd.arg("--manifest-path").arg(&manifest_path);
        } else {
            cmd.current_dir(&self.target_path);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let combined = format!("{stdout}\n{stderr}");
        let mut diagnostics = parse_fmt_output(&combined, &self.target_path);

        if !output.status.success() && diagnostics.is_empty() {
            let details = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else if !stdout.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                "Formatting check failed with non-zero exit code".to_string()
            };

            diagnostics.push(FmtDiagnostic::ExecutionFailure { details });
        }

        Ok(diagnostics)
    }
}

fn parse_fmt_diff_header(line: &str) -> Option<(&str, usize)> {
    let rest = line.trim().strip_prefix("Diff in ")?;
    if let Some((file_part, line_part)) = rest.split_once(" at line ") {
        let line_num = line_part.trim_suffix(':').trim().parse().unwrap_or(1);
        return Some((file_part.trim(), line_num));
    }
    let (file_part, line_part) = rest.trim_suffix(':').rsplit_once(':')?;
    let line_num = line_part.trim().parse().ok()?;
    Some((file_part.trim(), line_num))
}

/// Parses the output of `cargo fmt --check` into structured diagnostics.
pub fn parse_fmt_output(output: &str, target_dir: &Path) -> Vec<FmtDiagnostic> {
    let mut diagnostics = Vec::new();

    for line in output.lines() {
        // Supports both "Diff in /path/to/file.rs:10:" (Rust 1.100+) and
        // legacy "Diff in /path/to/file.rs at line 10:" rustfmt output.
        if let Some((file_str, line_num)) = parse_fmt_diff_header(line) {
            let normalized_path = normalize_path(Path::new(file_str), target_dir);
            let span = Span::new(normalized_path, line_num, 1, line_num, 1);

            diagnostics.push(FmtDiagnostic::FormattingDiff {
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
    fn parse_clean_fmt_output_returns_empty_diagnostics() -> Result<(), Box<dyn std::error::Error>>
    {
        let diags = parse_fmt_output("", Path::new("."));
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn parse_diff_fmt_output_extracts_file_and_line_span() -> Result<(), Box<dyn std::error::Error>>
    {
        let output = r#"
Diff in /workspace/src/lib.rs at line 25:
-fn test() {
+fn test() {
Diff in /workspace/src/main.rs at line 12:
-let a=1;
+let a = 1;
"#;
        let diags = parse_fmt_output(output, Path::new("/workspace"));
        assert_that!(diags.len(), eq(2));

        let first = diags.first().ok_or("expected first diagnostic")?;
        assert_that!(
            first.code().map(|c| c.to_string()),
            some(eq("fmt::formatting"))
        );
        assert_that!(first.severity(), some(eq(Severity::Warning)));
        assert_that!(
            first.to_string(),
            contains_substring("Source code formatting in '/workspace/src/lib.rs'")
        );

        let span = first.span().ok_or("expected span")?;
        assert_that!(span.file, eq(&PathBuf::from("src/lib.rs")));
        assert_that!(span.start_line, eq(25));

        let second = diags.get(1).ok_or("expected second diagnostic")?;
        let span2 = second.span().ok_or("expected span")?;
        assert_that!(span2.file, eq(&PathBuf::from("src/main.rs")));
        assert_that!(span2.start_line, eq(12));
        Ok(())
    }

    #[googletest::test]
    fn parse_relative_diff_fmt_output_preserves_path() -> Result<(), Box<dyn std::error::Error>> {
        let output = "Diff in src/foo.rs at line 42:\n";
        let diags = parse_fmt_output(output, Path::new("."));
        assert_that!(diags.len(), eq(1));

        let diag = diags.first().ok_or("expected diagnostic")?;
        let span = diag.span().ok_or("expected span")?;
        assert_that!(span.file, eq(&PathBuf::from("src/foo.rs")));
        assert_that!(span.start_line, eq(42));
        Ok(())
    }

    #[googletest::test]
    fn parse_colon_diff_fmt_output_extracts_file_and_line_span()
    -> Result<(), Box<dyn std::error::Error>> {
        let output = "\
Diff in /workspace/src/lib.rs:25:
-fn test() {
+fn test() {
Diff in src/main.rs:12:
-let a=1;
+let a = 1;
";
        let diags = parse_fmt_output(output, Path::new("/workspace"));
        assert_that!(diags.len(), eq(2));

        let first = diags.first().ok_or("expected first diagnostic")?;
        let span1 = first.span().ok_or("expected first span")?;
        assert_that!(span1.file, eq(&PathBuf::from("src/lib.rs")));
        assert_that!(span1.start_line, eq(25));

        let second = diags.get(1).ok_or("expected second diagnostic")?;
        let span2 = second.span().ok_or("expected second span")?;
        assert_that!(span2.file, eq(&PathBuf::from("src/main.rs")));
        assert_that!(span2.start_line, eq(12));
        Ok(())
    }
}
