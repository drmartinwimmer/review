use crate::tools::{
    ApiDiagnostic, AuditDiagnostic, ClippyDiagnostic, FmtDiagnostic, JsonDiagnostic,
    MarkdownDiagnostic, TomlDiagnostic,
};
pub use miette::{Diagnostic, Severity};
use miette::{LabeledSpan, SourceSpan};
pub use purist::{PuristDiagnostic, Span};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

pub(crate) fn span_labels(span: &Span) -> Box<dyn Iterator<Item = LabeledSpan> + '_> {
    Box::new(std::iter::once(LabeledSpan::underline(SourceSpan::from(
        span,
    ))))
}

/// Top-level `miette::Diagnostic` enum encompassing all findings produced by `code-review-check`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, miette::Diagnostic)]
pub enum CheckDiagnostic {
    /// Formatting violation from `cargo fmt`.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Fmt(#[from] FmtDiagnostic),

    /// Compiler or Clippy diagnostic from `cargo clippy`.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Clippy(#[from] ClippyDiagnostic),

    /// AST static analysis diagnostic from `purist`.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Purist(#[from] PuristDiagnostic),

    /// Dependency security advisory or warning from `cargo audit`.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Audit(#[from] AuditDiagnostic),

    /// Markdown formatting or syntax diagnostic.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Markdown(#[from] MarkdownDiagnostic),

    /// TOML formatting or syntax diagnostic.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Toml(#[from] TomlDiagnostic),

    /// JSON formatting or syntax diagnostic.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Json(#[from] JsonDiagnostic),

    /// Public API manifest drift diagnostic from `api-manifest`.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Api(#[from] ApiDiagnostic),
}

impl CheckDiagnostic {
    /// Returns the source [`Span`] associated with this diagnostic, if anchored to a file.
    pub fn span(&self) -> Option<&Span> {
        match self {
            Self::Fmt(diag) => diag.span(),
            Self::Clippy(diag) => diag.span(),
            Self::Purist(diag) => {
                let span = diag.span();
                if span.file.as_os_str().is_empty() {
                    None
                } else {
                    Some(span)
                }
            }
            Self::Audit(diag) => diag.span(),
            Self::Markdown(diag) => Some(diag.span()),
            Self::Toml(diag) => Some(diag.span()),
            Self::Json(diag) => Some(diag.span()),
            Self::Api(diag) => diag.span(),
        }
    }
}

/// Summary metrics aggregated for a diagnostic report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct ReportSummary {
    pub(crate) total_errors: usize,
    pub(crate) total_warnings: usize,
    pub(crate) total_advice: usize,
}

#[derive(Serialize)]
struct SerializedDiagnostic<'a> {
    rule: String,
    severity: Severity,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    span: Option<&'a Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_fix: Option<String>,
}

/// An aggregated container of all [`CheckDiagnostic`] findings produced during a check run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagnosticReport {
    pub diagnostics: Vec<CheckDiagnostic>,
    pub(crate) summary: ReportSummary,
}

impl Serialize for DiagnosticReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let serialized_diags: Vec<SerializedDiagnostic<'_>> = self
            .diagnostics
            .iter()
            .map(|d| {
                let rule = d
                    .code()
                    .map_or_else(|| "check::unknown".to_string(), |c| c.to_string());
                let severity = d.severity().unwrap_or(Severity::Warning);
                let suggested_fix = d.help().map(|h| h.to_string());
                SerializedDiagnostic {
                    rule,
                    severity,
                    message: d.to_string(),
                    span: d.span(),
                    suggested_fix,
                }
            })
            .collect();

        let mut state = serializer.serialize_struct("DiagnosticReport", 2)?;
        state.serialize_field("diagnostics", &serialized_diags)?;
        state.serialize_field("summary", &self.summary)?;
        state.end()
    }
}

impl DiagnosticReport {
    /// Creates a new diagnostic report with the given diagnostics, automatically computing summary counts.
    pub fn new(diagnostics: Vec<CheckDiagnostic>) -> Self {
        let mut report = Self::default();
        report.diagnostics.reserve(diagnostics.len());
        for diag in diagnostics {
            report.add(diag);
        }
        report
    }

    /// Returns true if any diagnostic has error severity.
    pub fn has_errors(&self) -> bool {
        self.summary.total_errors > 0
    }

    /// Returns the total count of error diagnostics.
    pub fn error_count(&self) -> usize {
        self.summary.total_errors
    }

    /// Returns the total count of warning diagnostics.
    pub fn warning_count(&self) -> usize {
        self.summary.total_warnings
    }

    /// Returns the total count of advice/hint diagnostics.
    pub fn advice_count(&self) -> usize {
        self.summary.total_advice
    }

    /// Returns true if there are no diagnostics in the report.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Adds a diagnostic to the report and updates summary metrics.
    pub fn add(&mut self, diag: impl Into<CheckDiagnostic>) {
        let diag = diag.into();
        match diag.severity().unwrap_or(Severity::Warning) {
            Severity::Error => self.summary.total_errors += 1,
            Severity::Warning => self.summary.total_warnings += 1,
            Severity::Advice => self.summary.total_advice += 1,
        }
        self.diagnostics.push(diag);
    }
}

impl<D: Into<CheckDiagnostic>> Extend<D> for DiagnosticReport {
    fn extend<T: IntoIterator<Item = D>>(&mut self, iter: T) {
        for item in iter {
            self.add(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::PathBuf;

    #[googletest::test]
    fn check_empty_report_returns_zero_counts() {
        let report = DiagnosticReport::default();
        expect_that!(report.is_empty(), is_true());
        expect_that!(report.has_errors(), is_false());
        expect_that!(report.error_count(), eq(0));
        expect_that!(report.warning_count(), eq(0));
        expect_that!(report.advice_count(), eq(0));
    }

    #[googletest::test]
    fn extend_report_aggregates_counts_and_miette_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut report = DiagnosticReport::default();
        let fmt_span = Span::new("src/lib.rs", 10, 1, 10, 1);
        report.add(FmtDiagnostic::FormattingDiff {
            file: "src/lib.rs".to_string(),
            span: fmt_span.clone(),
        });
        report.add(JsonDiagnostic::SyntaxError {
            file: "config.json".to_string(),
            details: "unexpected EOF".to_string(),
            span: Span::new("config.json", 3, 5, 3, 5),
        });
        report.add(AuditDiagnostic::Skipped {
            reason: "No Cargo.lock found".to_string(),
        });

        expect_that!(report.is_empty(), is_false());
        expect_that!(report.has_errors(), is_true());
        expect_that!(report.error_count(), eq(1));
        expect_that!(report.warning_count(), eq(1));
        expect_that!(report.advice_count(), eq(1));
        expect_that!(report.diagnostics.len(), eq(3));

        let first = report.diagnostics.first().ok_or("missing first")?;
        expect_that!(
            first.code().map(|c| c.to_string()),
            some(eq("fmt::formatting"))
        );
        expect_that!(first.span(), some(eq(&fmt_span)));
        expect_that!(
            first.help().map(|h| h.to_string()),
            some(contains_substring("cargo fmt"))
        );

        let third = report.diagnostics.get(2).ok_or("missing third")?;
        expect_that!(third.span(), none());
        expect_that!(third.severity(), some(eq(Severity::Advice)));

        let serialized = serde_json::to_value(&report)?;
        expect_that!(
            serialized
                .get("summary")
                .and_then(|s| s.get("total_errors"))
                .and_then(serde_json::Value::as_u64),
            some(eq(1))
        );
        expect_that!(
            serialized
                .get("diagnostics")
                .and_then(|d| d.get(0))
                .and_then(|d| d.get("span"))
                .and_then(|s| s.get("file"))
                .and_then(serde_json::Value::as_str),
            some(eq(PathBuf::from("src/lib.rs").to_str().unwrap_or_default()))
        );
        Ok(())
    }

    #[googletest::test]
    fn flattened_check_diagnostic_delegates_miette_protocol() -> googletest::Result<()> {
        let span = Span::new("Cargo.lock", 1, 1, 1, 10).with_byte_offsets(0, 9);
        let audit = CheckDiagnostic::from(AuditDiagnostic::Vulnerability {
            package: "demo@0.1.0".to_string(),
            id: "RUSTSEC-2026-0001".to_string(),
            title: "demo vulnerability".to_string(),
            url: Some("https://rustsec.org/advisories/RUSTSEC-2026-0001".to_string()),
            fix: "Upgrade 'demo' to >=0.2.0".to_string(),
            span: span.clone(),
        });

        verify_that!(
            audit.code().map(|c| c.to_string()),
            some(eq("audit::vulnerability"))
        )?;
        verify_that!(audit.severity(), some(eq(Severity::Error)))?;
        verify_that!(
            audit.help().map(|h| h.to_string()),
            some(eq("Upgrade 'demo' to >=0.2.0"))
        )?;
        verify_that!(
            audit.url().map(|u| u.to_string()),
            some(eq("https://rustsec.org/advisories/RUSTSEC-2026-0001"))
        )?;
        verify_that!(audit.span(), some(eq(&span)))?;
        verify_that!(audit.source_code().is_some(), is_true())?;
        verify_that!(audit.labels().map(Iterator::count), some(eq(1)))?;

        let clippy = CheckDiagnostic::from(ClippyDiagnostic::CompilerMessage {
            code: "clippy::unwrap_used".to_string(),
            severity: Severity::Warning,
            message: "used `unwrap()` on an `Option` value".to_string(),
            span: Some(Span::new("src/main.rs", 5, 9, 5, 17).with_byte_offsets(40, 48)),
            suggested_fix: Some("use `?` instead".to_string()),
        });
        verify_that!(
            clippy.code().map(|c| c.to_string()),
            some(eq("clippy::unwrap_used"))
        )?;
        verify_that!(clippy.source_code().is_some(), is_true())?;
        verify_that!(clippy.labels().map(Iterator::count), some(eq(1)))?;
        Ok(())
    }
}
