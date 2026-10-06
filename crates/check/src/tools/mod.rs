pub mod api;
pub mod cargo_audit;
pub mod cargo_clippy;
pub mod cargo_fmt;
pub mod file_utils;
pub mod json;
pub mod markdown;
pub mod purist;
pub mod toml;
pub mod vcs_jj;

pub use api::ApiRunner;
pub use cargo_audit::AuditRunner;
pub use cargo_clippy::ClippyRunner;
pub use cargo_fmt::FmtRunner;
pub use json::JsonRunner;
pub use markdown::MarkdownRunner;
pub use purist::{OpinionatedRunner, PuristRunner};
pub use toml::TomlRunner;
pub use vcs_jj::{JjError, JjVcs, filter_diagnostics_by_changed_files};

use ::purist::{Diagnostic, DiagnosticReport};

/// Aggregates diagnostics from multiple checking tools into a consolidated `DiagnosticReport`.
pub fn aggregate_diagnostics(
    fmt_diags: Vec<Diagnostic>,
    clippy_diags: Vec<Diagnostic>,
    purist_report: DiagnosticReport,
    audit_diags: Vec<Diagnostic>,
    markdown_diags: Vec<Diagnostic>,
    toml_diags: Vec<Diagnostic>,
    json_diags: Vec<Diagnostic>,
) -> DiagnosticReport {
    let mut all = Vec::new();
    all.extend(fmt_diags);
    all.extend(clippy_diags);
    all.extend(purist_report.diagnostics);
    all.extend(audit_diags);
    all.extend(markdown_diags);
    all.extend(toml_diags);
    all.extend(json_diags);
    DiagnosticReport::new(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::purist::Severity;
    use googletest::prelude::*;

    #[googletest::test]
    fn aggregate_diagnostics_combines_all_sources_and_computes_summary()
    -> Result<(), Box<dyn std::error::Error>> {
        let fmt = vec![Diagnostic::new(
            "fmt::formatting",
            Severity::Warning,
            "bad fmt",
        )];
        let clippy = vec![Diagnostic::new(
            "clippy::foo",
            Severity::Error,
            "bad clippy",
        )];
        let purist = DiagnosticReport::new(vec![Diagnostic::new(
            "purist::rule",
            Severity::Warning,
            "bad purist",
        )]);
        let audit = vec![Diagnostic::new(
            "audit::vuln",
            Severity::Error,
            "vulnerability",
        )];
        let md = vec![Diagnostic::new(
            "fmt::markdown",
            Severity::Warning,
            "bad md",
        )];
        let toml = vec![Diagnostic::new("fmt::toml", Severity::Warning, "bad toml")];
        let json = vec![Diagnostic::new("json::syntax", Severity::Error, "bad json")];

        let report = aggregate_diagnostics(fmt, clippy, purist, audit, md, toml, json);
        assert_that!(report.diagnostics.len(), eq(7));
        assert_that!(report.error_count(), eq(3));
        assert_that!(report.warning_count(), eq(4));
        assert_that!(report.has_errors(), is_true());
        Ok(())
    }
}
