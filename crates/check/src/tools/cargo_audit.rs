use crate::diagnostics::{Diagnostic, Severity, Span, span_labels};
use miette::{LabeledSpan, SourceCode};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Diagnostics emitted by [`AuditRunner`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuditDiagnostic {
    /// Security vulnerability advisory affecting a locked dependency.
    #[error("Security vulnerability in {package}: {title} ({id})")]
    Vulnerability {
        package: String,
        id: String,
        title: String,
        url: Option<String>,
        fix: String,
        span: Span,
    },

    /// Non-vulnerability advisory warning (e.g., `unmaintained`, `unsound`, `yanked`).
    #[error("Security warning [{kind}] for {package}: {title} ({id})")]
    AdvisoryWarning {
        kind: String,
        package: String,
        id: String,
        title: String,
        span: Span,
    },

    /// `cargo audit` could not run because `Cargo.lock` or `cargo-audit` was absent.
    #[error("{reason}")]
    Skipped { reason: String },

    /// `cargo audit` exited with an execution error.
    #[error("{details}")]
    ExecutionFailure { details: String },
}

impl AuditDiagnostic {
    /// Returns the source [`Span`] associated with this diagnostic, if any.
    pub const fn span(&self) -> Option<&Span> {
        match self {
            Self::Vulnerability { span, .. } | Self::AdvisoryWarning { span, .. } => Some(span),
            Self::Skipped { .. } | Self::ExecutionFailure { .. } => None,
        }
    }
}

impl Diagnostic for AuditDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        match self {
            Self::Vulnerability { .. } => Some(Box::new("audit::vulnerability")),
            Self::AdvisoryWarning { kind, .. } => Some(Box::new(format!("audit::{kind}"))),
            Self::Skipped { .. } => Some(Box::new("audit::skipped")),
            Self::ExecutionFailure { .. } => Some(Box::new("audit::error")),
        }
    }

    fn severity(&self) -> Option<Severity> {
        match self {
            Self::Vulnerability { .. } => Some(Severity::Error),
            Self::AdvisoryWarning { .. } | Self::ExecutionFailure { .. } => Some(Severity::Warning),
            Self::Skipped { .. } => Some(Severity::Advice),
        }
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        match self {
            Self::Vulnerability { fix, .. } => Some(Box::new(fix)),
            Self::AdvisoryWarning { .. } | Self::Skipped { .. } | Self::ExecutionFailure { .. } => {
                None
            }
        }
    }

    fn url<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        match self {
            Self::Vulnerability { url, .. } => {
                url.as_deref().map(|u| Box::new(u) as Box<dyn fmt::Display>)
            }
            Self::AdvisoryWarning { .. } | Self::Skipped { .. } | Self::ExecutionFailure { .. } => {
                None
            }
        }
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.span().map(|s| s as &dyn SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.span().map(span_labels)
    }
}

/// Runner for `cargo audit --json`.
pub struct AuditRunner {
    target_path: PathBuf,
}

impl AuditRunner {
    /// Creates a new `AuditRunner` targeting the specified directory or workspace.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes `cargo audit` and parses advisories into diagnostics.
    pub fn run(&self) -> Result<Vec<AuditDiagnostic>, io::Error> {
        let lock_file = if self.target_path.is_file() {
            self.target_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("Cargo.lock")
        } else {
            self.target_path.join("Cargo.lock")
        };

        if !lock_file.exists() {
            return Ok(vec![AuditDiagnostic::Skipped {
                reason: "No Cargo.lock found in target directory; skipping security audit"
                    .to_string(),
            }]);
        }

        let output = match self.invoke_audit(true) {
            Ok(out) => out,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(vec![AuditDiagnostic::Skipped {
                    reason:
                        "cargo-audit binary is not installed; skipping dependency security scan"
                            .to_string(),
                }]);
            }
            Err(err) => return Err(err),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // If fetch/lock failed, retry offline without fetching (--no-fetch)
        if !output.status.success() && stdout.trim().is_empty() {
            if let Ok(retry_output) = self.invoke_audit(false) {
                let retry_stdout = String::from_utf8_lossy(&retry_output.stdout);
                if !retry_stdout.trim().is_empty() {
                    return Ok(parse_audit_json(&retry_stdout, &lock_file));
                }
            }

            let details = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                "cargo audit execution failed".to_string()
            };
            return Ok(vec![AuditDiagnostic::ExecutionFailure { details }]);
        }

        Ok(parse_audit_json(&stdout, &lock_file))
    }

    fn invoke_audit(&self, fetch: bool) -> Result<std::process::Output, io::Error> {
        let mut cmd = Command::new("cargo");
        cmd.arg("audit").arg("--json");

        if !fetch {
            cmd.arg("--no-fetch");
        }

        if self.target_path.is_dir() {
            cmd.current_dir(&self.target_path);
        } else if let Some(parent) = self.target_path.parent() {
            cmd.current_dir(parent);
        }

        cmd.output()
    }
}

#[derive(Debug, Deserialize, Default)]
struct AuditOutput {
    #[serde(default)]
    vulnerabilities: AuditVulnerabilities,
    #[serde(default)]
    warnings: BTreeMap<String, Vec<AuditWarningItem>>,
}

#[derive(Debug, Deserialize, Default)]
struct AuditVulnerabilities {
    #[serde(default)]
    list: Vec<AuditVulnerabilityItem>,
}

#[derive(Debug, Deserialize)]
struct AuditVulnerabilityItem {
    advisory: AuditAdvisory,
    package: AuditPackage,
    #[serde(default)]
    versions: Option<AuditVersions>,
}

#[derive(Debug, Deserialize)]
struct AuditAdvisory {
    id: String,
    title: String,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuditPackage {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct AuditVersions {
    #[serde(default)]
    patched: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AuditWarningItem {
    #[serde(default)]
    package: Option<AuditPackage>,
    #[serde(default)]
    advisory: Option<AuditAdvisory>,
}

/// Parses the JSON output of `cargo audit` into diagnostics.
pub fn parse_audit_json(json_str: &str, lock_file: &Path) -> Vec<AuditDiagnostic> {
    let mut diagnostics = Vec::new();
    let trimmed = json_str.trim();

    if trimmed.is_empty() {
        return diagnostics;
    }

    let Ok(audit) = serde_json::from_str::<AuditOutput>(trimmed) else {
        return diagnostics;
    };

    let span = Span::new(lock_file.to_path_buf(), 1, 1, 1, 1);

    for vuln in audit.vulnerabilities.list {
        let pkg = format!("{}@{}", vuln.package.name, vuln.package.version);
        let id = vuln.advisory.id;
        let title = vuln.advisory.title;
        let url = vuln.advisory.url;

        let fix = if let Some(versions) = &vuln.versions
            && !versions.patched.is_empty()
        {
            format!(
                "Upgrade '{}' to {}",
                vuln.package.name,
                versions.patched.join(", ")
            )
        } else {
            format!("Consult advisory at {}", url.as_deref().unwrap_or(&id))
        };

        diagnostics.push(AuditDiagnostic::Vulnerability {
            package: pkg,
            id,
            title,
            url,
            fix,
            span: span.clone(),
        });
    }

    for (kind, warnings) in audit.warnings {
        for warn in warnings {
            let package = warn.package.as_ref().map_or_else(
                || "dependency".to_string(),
                |p| format!("{}@{}", p.name, p.version),
            );
            let id = warn
                .advisory
                .as_ref()
                .map_or_else(|| "notice".to_string(), |a| a.id.clone());
            let title = warn.advisory.as_ref().map_or_else(
                || "dependency advisory notice".to_string(),
                |a| a.title.clone(),
            );

            diagnostics.push(AuditDiagnostic::AdvisoryWarning {
                kind: kind.clone(),
                package,
                id,
                title,
                span: span.clone(),
            });
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn parse_clean_audit_json_returns_empty_diagnostics() -> Result<(), Box<dyn std::error::Error>>
    {
        let json = r#"
{"database":{"advisory-count":1200},"lockfile":{"dependency-count":50},"settings":{},"vulnerabilities":{"found":false,"count":0,"list":[]},"warnings":{}}
"#;
        let diags = parse_audit_json(json, Path::new("Cargo.lock"));
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn parse_vulnerability_audit_json_extracts_diagnostics_with_span_and_fix()
    -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"
{
  "vulnerabilities": {
    "found": true,
    "count": 1,
    "list": [
      {
        "advisory": {
          "id": "RUSTSEC-2020-0071",
          "title": "Potential segfault in time::parse",
          "url": "https://rustsec.org/advisories/RUSTSEC-2020-0071"
        },
        "package": {
          "name": "time",
          "version": "0.1.43"
        },
        "versions": {
          "patched": [">=0.2.23"]
        }
      }
    ]
  },
  "warnings": {}
}
"#;
        let diags = parse_audit_json(json, Path::new("Cargo.lock"));
        assert_that!(diags.len(), eq(1));

        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            diag.code().map(|c| c.to_string()),
            some(eq("audit::vulnerability"))
        );
        assert_that!(diag.severity(), some(eq(Severity::Error)));
        assert_that!(
            diag.to_string(),
            contains_substring(
                "Security vulnerability in time@0.1.43: Potential segfault in time::parse (RUSTSEC-2020-0071)"
            )
        );
        assert_that!(
            diag.help().map(|h| h.to_string()),
            some(eq("Upgrade 'time' to >=0.2.23"))
        );
        assert_that!(
            diag.url().map(|u| u.to_string()),
            some(eq("https://rustsec.org/advisories/RUSTSEC-2020-0071"))
        );

        let span = diag.span().ok_or("expected span")?;
        assert_that!(span.file, eq(&PathBuf::from("Cargo.lock")));
        Ok(())
    }

    #[googletest::test]
    fn parse_unmaintained_warning_extracts_warning_diagnostic()
    -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"
{
  "vulnerabilities": { "found": false, "count": 0, "list": [] },
  "warnings": {
    "unmaintained": [
      {
        "package": { "name": "net2", "version": "0.2.37" },
        "advisory": {
          "id": "RUSTSEC-2020-0016",
          "title": "net2 is unmaintained"
        }
      }
    ]
  }
}
"#;
        let diags = parse_audit_json(json, Path::new("Cargo.lock"));
        assert_that!(diags.len(), eq(1));

        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            diag.code().map(|c| c.to_string()),
            some(eq("audit::unmaintained"))
        );
        assert_that!(diag.severity(), some(eq(Severity::Warning)));
        assert_that!(
            diag.to_string(),
            contains_substring(
                "Security warning [unmaintained] for net2@0.2.37: net2 is unmaintained (RUSTSEC-2020-0016)"
            )
        );
        Ok(())
    }
}
