use crate::diagnostics::{CheckDiagnostic, Diagnostic, DiagnosticReport, Severity};
use annotate_snippets::{Group, Level, Origin, Renderer};
use anstyle::Style;
pub use purist::OutputFormat;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

/// Renders a diagnostic report to the specified writer in the chosen format.
/// Automatically detects color support based on standard environment and terminal conventions.
pub fn render_report(
    report: &DiagnosticReport,
    format: OutputFormat,
    writer: &mut dyn io::Write,
) -> io::Result<()> {
    let use_color = auto_detect_color();
    render_report_with_options(report, format, writer, use_color)
}

fn auto_detect_color() -> bool {
    if anstyle_query::no_color() {
        return false;
    }
    if anstyle_query::clicolor_force() {
        return true;
    }
    if anstyle_query::clicolor() == Some(false) {
        return false;
    }
    true
}

pub(crate) fn render_report_with_options(
    report: &DiagnosticReport,
    format: OutputFormat,
    writer: &mut dyn io::Write,
    use_color: bool,
) -> io::Result<()> {
    match format {
        OutputFormat::Console => render_console(report, writer, use_color),
        OutputFormat::Json => render_json(report, writer),
        OutputFormat::Markdown => render_markdown(report, writer),
    }
}

fn render_console(
    report: &DiagnosticReport,
    writer: &mut dyn io::Write,
    use_color: bool,
) -> io::Result<()> {
    if report.is_empty() {
        writeln!(writer, "No issues found.")?;
        return Ok(());
    }

    let renderer = if use_color {
        Renderer::styled()
    } else {
        Renderer::plain()
    };

    let mut by_file: BTreeMap<Option<&Path>, Vec<&CheckDiagnostic>> = BTreeMap::new();
    for diag in &report.diagnostics {
        let file = diag.span().map(|s| s.file.as_path());
        by_file.entry(file).or_default().push(diag);
    }

    for (file_opt, diags) in by_file {
        let path_str = file_opt.map(|f| f.display().to_string());
        for diag in diags {
            let level = match diag.severity().unwrap_or(Severity::Warning) {
                Severity::Error => Level::ERROR,
                Severity::Warning => Level::WARNING,
                Severity::Advice => Level::HELP,
            };

            let message = diag.to_string();
            let rule = diag
                .code()
                .map_or_else(|| "check::unknown".to_string(), |c| c.to_string());
            let help_str = diag.help().map(|h| h.to_string());

            let title = level.primary_title(&message).id(&rule);
            let mut group = Group::with_title(title);

            if let (Some(path), Some(span)) = (&path_str, diag.span())
                && !path.is_empty()
            {
                let origin = Origin::path(path)
                    .line(span.start_line)
                    .char_column(span.start_col);
                group = group.element(origin);
            }

            if let Some(fix) = &help_str {
                group = group.element(Level::HELP.message(fix));
            }

            let rendered = renderer.render(&[group]);
            writeln!(writer, "{rendered}")?;
        }
    }

    let summary_line = format!(
        "{} error(s), {} warning(s), 0 info, {} hint(s) found.",
        report.error_count(),
        report.warning_count(),
        report.advice_count()
    );

    if use_color {
        let bold = Style::new().bold();
        writeln!(writer, "{bold}{summary_line}{bold:#}")?;
    } else {
        writeln!(writer, "{summary_line}")?;
    }

    Ok(())
}

fn render_json(report: &DiagnosticReport, writer: &mut dyn io::Write) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *writer, report).map_err(io::Error::other)?;
    writeln!(writer)?;
    Ok(())
}

fn render_markdown(report: &DiagnosticReport, writer: &mut dyn io::Write) -> io::Result<()> {
    writeln!(writer, "# Diagnostic Report\n")?;

    if report.is_empty() {
        writeln!(writer, "No issues found.")?;
        return Ok(());
    }

    writeln!(
        writer,
        "| Severity | Rule | Location | Message | Suggestion |"
    )?;
    writeln!(writer, "| --- | --- | --- | --- | --- |")?;

    for diag in &report.diagnostics {
        let location = match diag.span() {
            Some(span) if !span.file.as_os_str().is_empty() => format!(
                "`{}:{}:{}`",
                span.file.display(),
                span.start_line,
                span.start_col
            ),
            _ => "-".to_string(),
        };

        let suggestion = match diag.help().map(|h| h.to_string()) {
            Some(fix) => {
                let sanitized_fix = fix.replace('|', "\\|").replace('\n', " ");
                format!("`{sanitized_fix}`")
            }
            None => "-".to_string(),
        };

        let sanitized_msg = diag.to_string().replace('|', "\\|").replace('\n', " ");
        let rule = diag
            .code()
            .map_or_else(|| "check::unknown".to_string(), |c| c.to_string());
        let severity = diag.severity().unwrap_or(Severity::Warning);

        writeln!(
            writer,
            "| {:?} | `{}` | {} | {} | {} |",
            severity, rule, location, sanitized_msg, suggestion
        )?;
    }

    writeln!(
        writer,
        "\n**Summary**: {} error(s), {} warning(s), 0 info, {} hint(s)",
        report.error_count(),
        report.warning_count(),
        report.advice_count()
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Span;
    use crate::tools::{AuditDiagnostic, FmtDiagnostic, TomlDiagnostic};
    use googletest::prelude::*;

    #[googletest::test]
    fn render_console_empty_report_outputs_no_issues() -> Result<(), Box<dyn std::error::Error>> {
        let report = DiagnosticReport::default();
        let mut buffer = Vec::new();
        render_report_with_options(&report, OutputFormat::Console, &mut buffer, false)?;
        let output = String::from_utf8(buffer)?;
        expect_that!(output, contains_substring("No issues found."));
        Ok(())
    }

    #[googletest::test]
    fn render_console_formats_grouped_diagnostics_and_summary()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut report = DiagnosticReport::default();
        report.add(TomlDiagnostic::SyntaxError {
            file: "Cargo.toml".to_string(),
            details: "unexpected character".to_string(),
            span: Span::new("Cargo.toml", 4, 2, 4, 2),
        });
        report.add(FmtDiagnostic::FormattingDiff {
            file: "src/main.rs".to_string(),
            span: Span::new("src/main.rs", 12, 1, 12, 1),
        });
        report.add(AuditDiagnostic::Skipped {
            reason: "No Cargo.lock found".to_string(),
        });

        let mut buffer = Vec::new();
        render_report_with_options(&report, OutputFormat::Console, &mut buffer, false)?;
        let output = String::from_utf8(buffer)?;

        expect_that!(output, contains_substring("error[toml::syntax]"));
        expect_that!(output, contains_substring("--> Cargo.toml:4:2"));
        expect_that!(output, contains_substring("warning[fmt::formatting]"));
        expect_that!(output, contains_substring("--> src/main.rs:12:1"));
        expect_that!(output, contains_substring("help[audit::skipped]"));
        expect_that!(
            output,
            contains_substring("1 error(s), 1 warning(s), 0 info, 1 hint(s) found.")
        );
        Ok(())
    }

    #[googletest::test]
    fn render_json_and_markdown_serialize_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
        let mut report = DiagnosticReport::default();
        report.add(FmtDiagnostic::FormattingDiff {
            file: "src/lib.rs".to_string(),
            span: Span::new("src/lib.rs", 5, 1, 5, 1),
        });

        let mut json_buf = Vec::new();
        render_report(&report, OutputFormat::Json, &mut json_buf)?;
        let parsed: serde_json::Value = serde_json::from_slice(&json_buf)?;
        expect_that!(
            parsed
                .get("diagnostics")
                .and_then(|d| d.get(0))
                .and_then(|d| d.get("rule"))
                .and_then(serde_json::Value::as_str),
            some(eq("fmt::formatting"))
        );

        let mut md_buf = Vec::new();
        render_report(&report, OutputFormat::Markdown, &mut md_buf)?;
        let md = String::from_utf8(md_buf)?;
        expect_that!(
            md,
            contains_substring("| Warning | `fmt::formatting` | `src/lib.rs:5:1` |")
        );
        expect_that!(md, contains_substring("`Run 'cargo fmt' to format"));
        Ok(())
    }
}
