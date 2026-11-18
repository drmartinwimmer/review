use super::file_utils::normalize_path;
use crate::diagnostics::{Diagnostic, Severity, Span, span_labels};
use miette::{LabeledSpan, SourceCode};
use serde::Deserialize;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Diagnostics emitted by [`ClippyRunner`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClippyDiagnostic {
    /// A compiler, Clippy, or Cargo manifest lint diagnostic.
    #[error("{message}")]
    CompilerMessage {
        code: String,
        severity: Severity,
        message: String,
        span: Option<Span>,
        suggested_fix: Option<String>,
    },

    /// `cargo clippy` failed with a non-zero exit code without emitting compiler JSON diagnostics.
    #[error("{details}")]
    ExecutionFailure { details: String },
}

impl ClippyDiagnostic {
    /// Returns the source [`Span`] associated with this diagnostic, if any.
    pub const fn span(&self) -> Option<&Span> {
        match self {
            Self::CompilerMessage { span, .. } => span.as_ref(),
            Self::ExecutionFailure { .. } => None,
        }
    }
}

impl Diagnostic for ClippyDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        match self {
            Self::CompilerMessage { code, .. } => Some(Box::new(code)),
            Self::ExecutionFailure { .. } => Some(Box::new("clippy::execution")),
        }
    }

    fn severity(&self) -> Option<Severity> {
        match self {
            Self::CompilerMessage { severity, .. } => Some(*severity),
            Self::ExecutionFailure { .. } => Some(Severity::Error),
        }
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        match self {
            Self::CompilerMessage { suggested_fix, .. } => suggested_fix
                .as_deref()
                .map(|fix| Box::new(fix) as Box<dyn fmt::Display>),
            Self::ExecutionFailure { .. } => None,
        }
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.span().map(|s| s as &dyn SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.span().map(span_labels)
    }
}

/// Runner for `cargo clippy --message-format=json`.
pub struct ClippyRunner {
    target_path: PathBuf,
}

impl ClippyRunner {
    /// Creates a new `ClippyRunner` targeting the specified directory or workspace.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes `cargo clippy` and parses compiler messages into diagnostics.
    pub fn run(&self) -> Result<Vec<ClippyDiagnostic>, io::Error> {
        let manifest_path = if self.target_path.is_file() {
            self.target_path.clone()
        } else {
            self.target_path.join("Cargo.toml")
        };

        let mut cmd = Command::new("cargo");
        cmd.arg("clippy")
            .arg("--all-targets")
            .arg("--all-features")
            .arg("--message-format=json");

        if manifest_path.exists() {
            cmd.arg("--manifest-path").arg(&manifest_path);
        } else {
            cmd.current_dir(&self.target_path);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut diagnostics = parse_cargo_lint_stderr(&stderr, &self.target_path);
        diagnostics.extend(parse_clippy_json_stream(&stdout, &self.target_path));

        if !output.status.success() && diagnostics.is_empty() {
            let details = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                "Clippy execution failed with non-zero exit code".to_string()
            };

            diagnostics.push(ClippyDiagnostic::ExecutionFailure { details });
        }

        Ok(diagnostics)
    }
}

#[derive(Debug, Deserialize)]
struct RustcMessageEnvelope {
    reason: String,
    #[serde(default)]
    message: Option<RustcMessage>,
}

#[derive(Debug, Deserialize)]
struct RustcMessage {
    message: String,
    code: Option<RustcCode>,
    level: String,
    #[serde(default)]
    spans: Vec<RustcSpan>,
    #[serde(default)]
    children: Vec<RustcChildMessage>,
}

#[derive(Debug, Deserialize)]
struct RustcCode {
    code: String,
}

#[derive(Debug, Deserialize)]
struct RustcSpan {
    file_name: String,
    byte_start: usize,
    byte_end: usize,
    line_start: usize,
    line_end: usize,
    column_start: usize,
    column_end: usize,
    is_primary: bool,
    suggested_replacement: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RustcChildMessage {
    message: String,
    level: String,
    #[serde(default)]
    spans: Vec<RustcSpan>,
}

/// Parses a stream of newline-delimited JSON compiler messages into diagnostics.
pub fn parse_clippy_json_stream(stream: &str, target_dir: &Path) -> Vec<ClippyDiagnostic> {
    let mut diagnostics = Vec::new();

    for line in stream.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(envelope) = serde_json::from_str::<RustcMessageEnvelope>(trimmed) else {
            continue;
        };

        if envelope.reason != "compiler-message" {
            continue;
        }

        let Some(msg) = envelope.message else {
            continue;
        };

        // Ignore compiler notes without actionable context (e.g. aborting due to previous error)
        if msg.level == "failure-note" {
            continue;
        }

        let severity = match msg.level.as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            _ => Severity::Advice,
        };

        let code = msg
            .code
            .as_ref()
            .map_or_else(|| format!("rustc::{}", msg.level), |c| c.code.clone());

        let primary_span = msg
            .spans
            .iter()
            .find(|s| s.is_primary)
            .or_else(|| msg.spans.first());

        let span = primary_span.map(|s| {
            let normalized_path = normalize_path(Path::new(&s.file_name), target_dir);

            Span::new(
                normalized_path,
                s.line_start,
                s.column_start,
                s.line_end,
                s.column_end,
            )
            .with_byte_offsets(s.byte_start, s.byte_end)
        });

        let suggested_fix = primary_span
            .and_then(|s| s.suggested_replacement.clone())
            .filter(|r| !r.is_empty())
            .or_else(|| extract_child_suggested_fix(&msg.children));

        diagnostics.push(ClippyDiagnostic::CompilerMessage {
            code,
            severity,
            message: msg.message,
            span,
            suggested_fix,
        });
    }

    diagnostics
}

fn extract_child_suggested_fix(children: &[RustcChildMessage]) -> Option<String> {
    let mut fallback_message = None;

    for child in children {
        if child.level != "help" {
            continue;
        }

        if let Some(child_span) = child.spans.iter().find(|s| s.is_primary)
            && let Some(replacement) = &child_span.suggested_replacement
            && !replacement.is_empty()
        {
            return Some(replacement.clone());
        }

        if fallback_message.is_none() && !child.message.is_empty() {
            fallback_message = Some(child.message.clone());
        }
    }

    fallback_message
}

struct RawCargoDiagnosticBlock<'a> {
    severity: Severity,
    message: &'a str,
    lines: Vec<&'a str>,
}

fn parse_cargo_header(line: &str) -> Option<(Severity, &str)> {
    if let Some(msg) = line.strip_prefix("warning: ") {
        Some((Severity::Warning, msg.trim()))
    } else {
        line.strip_prefix("error: ")
            .map(|msg| (Severity::Error, msg.trim()))
    }
}

fn extract_cargo_lint_rule(lines: &[&str]) -> Option<String> {
    for line in lines {
        if let Some(after_note) = line.trim().strip_prefix("= note: `")
            && let Some((rule, _)) = after_note.split_once('`')
            && rule.starts_with("cargo::")
        {
            return Some(rule.to_string());
        }
    }
    None
}

fn extract_cargo_lint_span(lines: &[&str], target_dir: &Path) -> Option<Span> {
    for line in lines {
        if let Some(loc) = line.trim().strip_prefix("--> ") {
            let (before_col, col_str) = loc.rsplit_once(':')?;
            let (file_str, line_str) = before_col.rsplit_once(':')?;
            let line_num: usize = line_str.trim().parse().ok()?;
            let col_num: usize = col_str.trim().parse().ok()?;
            let normalized = normalize_path(Path::new(file_str.trim()), target_dir);
            return Some(Span::new(normalized, line_num, col_num, line_num, col_num));
        }
    }
    None
}

fn extract_cargo_lint_help(lines: &[&str]) -> Option<String> {
    for line in lines {
        if let Some(help) = line.trim().strip_prefix("help: ")
            && !help.trim().is_empty()
        {
            return Some(help.trim().to_string());
        }
    }
    None
}

fn build_cargo_lint_diagnostic(
    block: RawCargoDiagnosticBlock<'_>,
    target_dir: &Path,
) -> Option<ClippyDiagnostic> {
    let code = extract_cargo_lint_rule(&block.lines)?;
    let span = extract_cargo_lint_span(&block.lines, target_dir);
    let suggested_fix = extract_cargo_lint_help(&block.lines);
    Some(ClippyDiagnostic::CompilerMessage {
        code,
        severity: block.severity,
        message: block.message.to_string(),
        span,
        suggested_fix,
    })
}

/// Parses Cargo's own `[lints.cargo]` (`cargo::*`) manifest diagnostics from stderr.
pub(crate) fn parse_cargo_lint_stderr(stderr: &str, target_dir: &Path) -> Vec<ClippyDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut current: Option<RawCargoDiagnosticBlock<'_>> = None;

    for line in stderr.lines() {
        if let Some((severity, message)) = parse_cargo_header(line) {
            if let Some(prev) = current.take()
                && let Some(diag) = build_cargo_lint_diagnostic(prev, target_dir)
            {
                diagnostics.push(diag);
            }
            current = Some(RawCargoDiagnosticBlock {
                severity,
                message,
                lines: Vec::new(),
            });
        } else if let Some(active) = &mut current {
            active.lines.push(line);
        }
    }

    if let Some(last) = current
        && let Some(diag) = build_cargo_lint_diagnostic(last, target_dir)
    {
        diagnostics.push(diag);
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn parse_empty_stream_yields_no_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
        let diags = parse_clippy_json_stream("", Path::new("."));
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn parse_non_compiler_message_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{"reason":"compiler-artifact","package_id":"foo 0.1.0","target":{}}"#;
        let diags = parse_clippy_json_stream(json, Path::new("."));
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn parse_clippy_warning_maps_rule_code_span_and_suggestion()
    -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"
{"reason":"compiler-message","package_id":"demo 0.1.0","target":{"name":"demo"},"message":{"rendered":"warning: redundant clone\n","children":[{"children":[],"code":null,"level":"help","message":"remove this call","spans":[{"byte_end":50,"byte_start":40,"column_end":18,"column_start":8,"file_name":"src/main.rs","is_primary":true,"line_end":5,"line_start":5,"suggested_replacement":"","suggestion_applicability":"MachineApplicable","text":[]}]}],"code":{"code":"clippy::redundant_clone","explanation":null},"level":"warning","message":"redundant clone","spans":[{"byte_end":50,"byte_start":40,"column_end":18,"column_start":8,"file_name":"src/main.rs","is_primary":true,"line_end":5,"line_start":5,"suggested_replacement":null,"suggestion_applicability":null,"text":[]}]}}
"#;
        let diags = parse_clippy_json_stream(json, Path::new("."));
        assert_that!(diags.len(), eq(1));

        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            diag.code().map(|c| c.to_string()),
            some(eq("clippy::redundant_clone"))
        );
        assert_that!(diag.severity(), some(eq(Severity::Warning)));
        assert_that!(diag.to_string(), eq("redundant clone"));

        let span = diag.span().ok_or("expected span")?;
        assert_that!(span.file, eq(&PathBuf::from("src/main.rs")));
        assert_that!(span.start_line, eq(5));
        assert_that!(span.start_col, eq(8));
        assert_that!(span.end_line, eq(5));
        assert_that!(span.end_col, eq(18));

        assert_that!(
            diag.help().map(|h| h.to_string()),
            some(eq("remove this call"))
        );
        Ok(())
    }

    #[googletest::test]
    fn parse_compiler_error_maps_error_severity() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"
{"reason":"compiler-message","message":{"code":{"code":"E0425"},"level":"error","message":"cannot find value `x` in this scope","spans":[{"byte_end":30,"byte_start":29,"column_end":6,"column_start":5,"file_name":"src/lib.rs","is_primary":true,"line_end":2,"line_start":2,"suggested_replacement":null}]}}
"#;
        let diags = parse_clippy_json_stream(json, Path::new("."));
        assert_that!(diags.len(), eq(1));

        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(diag.code().map(|c| c.to_string()), some(eq("E0425")));
        assert_that!(diag.severity(), some(eq(Severity::Error)));
        assert_that!(
            diag.to_string(),
            contains_substring("cannot find value `x` in this scope")
        );
        Ok(())
    }

    #[googletest::test]
    fn parse_failure_note_is_skipped() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"
{"reason":"compiler-message","message":{"code":null,"level":"failure-note","message":"aborting due to 1 previous error","spans":[]}}
"#;
        let diags = parse_clippy_json_stream(json, Path::new("."));
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn parse_cargo_lint_stderr_extracts_warnings_and_errors_with_spans()
    -> Result<(), Box<dyn std::error::Error>> {
        let stderr = "\
warning: package `bad_pkg` should have a kebab-case name
 --> crates/sub/Cargo.toml:2:8
  |
2 | name = \"bad_pkg\"
  |        ^^^^^^^^^
  |
  = note: `cargo::non_kebab_case_packages` is set to `warn` in `[lints]`
help: to change the package name to `bad-pkg`, convert `package.name`
  |
2 - name = \"bad_pkg\"
2 + name = \"bad-pkg\"
  |
warning: `bad_pkg` (manifest) generated 1 warning
    Checking bad_pkg v0.1.0 (/workspace)
error: unused dependency `itoa`
 --> /workspace/Cargo.toml:10:1
  |
10 | itoa = \"1.0\"
  | ^^^^^^^^^^^^
  |
  = note: `cargo::unused_dependencies` is set to `deny` in `[lints]`
help: consider removing the dependency on `itoa`
error: could not compile `bad_pkg` due to 1 previous error
";
        let diags = parse_cargo_lint_stderr(stderr, Path::new("/workspace"));
        assert_that!(diags.len(), eq(2));

        let first = diags.first().ok_or("expected first cargo lint")?;
        assert_that!(
            first.code().map(|c| c.to_string()),
            some(eq("cargo::non_kebab_case_packages"))
        );
        assert_that!(first.severity(), some(eq(Severity::Warning)));
        assert_that!(
            first.to_string(),
            eq("package `bad_pkg` should have a kebab-case name")
        );
        let span1 = first.span().ok_or("expected first span")?;
        assert_that!(span1.file, eq(&PathBuf::from("crates/sub/Cargo.toml")));
        assert_that!(span1.start_line, eq(2));
        assert_that!(span1.start_col, eq(8));
        assert_that!(
            first.help().map(|h| h.to_string()),
            some(eq(
                "to change the package name to `bad-pkg`, convert `package.name`"
            ))
        );

        let second = diags.get(1).ok_or("expected second cargo lint")?;
        assert_that!(
            second.code().map(|c| c.to_string()),
            some(eq("cargo::unused_dependencies"))
        );
        assert_that!(second.severity(), some(eq(Severity::Error)));
        assert_that!(second.to_string(), eq("unused dependency `itoa`"));
        let span2 = second.span().ok_or("expected second span")?;
        assert_that!(span2.file, eq(&PathBuf::from("Cargo.toml")));
        assert_that!(span2.start_line, eq(10));
        assert_that!(span2.start_col, eq(1));
        assert_that!(
            second.help().map(|h| h.to_string()),
            some(eq("consider removing the dependency on `itoa`"))
        );
        Ok(())
    }
}
