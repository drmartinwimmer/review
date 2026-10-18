use serde::{Deserialize, Serialize};
use std::io::Write;

/// Coverage metric counters and percentage for a specific dimension (lines, functions, branches, regions).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MetricSummary {
    /// Total number of coverable items.
    pub count: u64,
    /// Number of covered items.
    pub covered: u64,
    /// Coverage percentage (0.0 to 100.0).
    pub percent: f64,
}

/// Aggregated coverage percentages and counts across lines, functions, branches, and regions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CoverageSummary {
    /// Line coverage metrics.
    pub lines: MetricSummary,
    /// Function coverage metrics.
    pub functions: MetricSummary,
    /// Branch coverage metrics.
    pub branches: MetricSummary,
    /// Region coverage metrics.
    pub regions: MetricSummary,
}

/// Per-file coverage metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileCoverage {
    /// Source file path.
    pub filename: String,
    /// Coverage metrics for this file.
    pub summary: CoverageSummary,
}

/// Normalized coverage report extracted from `cargo llvm-cov --json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CoverageReport {
    /// Workspace or crate total coverage metrics.
    pub totals: CoverageSummary,
    /// Per-file coverage breakdowns sorted by filename.
    pub files: Vec<FileCoverage>,
}

impl CoverageReport {
    pub(crate) fn line_coverage_percent(&self) -> f64 {
        self.totals.lines.percent
    }

    #[cfg(test)]
    fn files_below_threshold(&self, threshold_pct: f64) -> Vec<&FileCoverage> {
        self.files
            .iter()
            .filter(|f| f.summary.lines.count > 0 && f.summary.lines.percent < threshold_pct)
            .collect()
    }

    pub(crate) fn render_console(&self, writer: &mut dyn Write) -> std::io::Result<()> {
        writeln!(
            writer,
            "{:<50} {:>10} {:>10} {:>10}",
            "File", "Lines %", "Funcs %", "Regions %"
        )?;
        writeln!(writer, "{}", "-".repeat(83))?;

        for file in &self.files {
            writeln!(
                writer,
                "{:<50} {:>9.2}% {:>9.2}% {:>9.2}%",
                file.filename,
                file.summary.lines.percent,
                file.summary.functions.percent,
                file.summary.regions.percent,
            )?;
        }

        writeln!(writer, "{}", "-".repeat(83))?;
        writeln!(
            writer,
            "{:<50} {:>9.2}% {:>9.2}% {:>9.2}%",
            "TOTAL",
            self.totals.lines.percent,
            self.totals.functions.percent,
            self.totals.regions.percent,
        )?;
        Ok(())
    }

    pub(crate) fn render_json(&self, writer: &mut dyn Write) -> Result<(), serde_json::Error> {
        serde_json::to_writer_pretty(writer, self)
    }

    pub(crate) fn format_lcov(&self) -> String {
        let mut out = String::new();
        for file in &self.files {
            out.push_str(&format!("SF:{}\n", file.filename));
            out.push_str(&format!("FNF:{}\n", file.summary.functions.count));
            out.push_str(&format!("FNH:{}\n", file.summary.functions.covered));
            out.push_str(&format!("LF:{}\n", file.summary.lines.count));
            out.push_str(&format!("LH:{}\n", file.summary.lines.covered));
            out.push_str("end_of_record\n");
        }
        out
    }
}

#[derive(Deserialize)]
struct RawLlvmCovExport {
    #[serde(default)]
    data: Vec<RawExportData>,
}

#[derive(Deserialize)]
struct RawExportData {
    #[serde(default)]
    totals: RawSummary,
    #[serde(default)]
    files: Vec<RawFileEntry>,
}

#[derive(Default, Deserialize)]
struct RawSummary {
    #[serde(default)]
    lines: RawMetric,
    #[serde(default)]
    functions: RawMetric,
    #[serde(default)]
    branches: RawMetric,
    #[serde(default)]
    regions: RawMetric,
}

#[derive(Default, Deserialize)]
struct RawMetric {
    #[serde(default)]
    count: u64,
    #[serde(default)]
    covered: u64,
    #[serde(default)]
    percent: f64,
}

#[derive(Deserialize)]
struct RawFileEntry {
    filename: String,
    #[serde(default)]
    summary: RawSummary,
}

impl From<RawMetric> for MetricSummary {
    fn from(raw: RawMetric) -> Self {
        Self {
            count: raw.count,
            covered: raw.covered,
            percent: raw.percent,
        }
    }
}

impl From<RawSummary> for CoverageSummary {
    fn from(raw: RawSummary) -> Self {
        Self {
            lines: raw.lines.into(),
            functions: raw.functions.into(),
            branches: raw.branches.into(),
            regions: raw.regions.into(),
        }
    }
}

pub(crate) fn parse_llvm_cov_json(json_str: &str) -> Result<CoverageReport, serde_json::Error> {
    let raw: RawLlvmCovExport = serde_json::from_str(json_str)?;
    let Some(first) = raw.data.into_iter().next() else {
        return Ok(CoverageReport::default());
    };

    let mut files: Vec<FileCoverage> = first
        .files
        .into_iter()
        .map(|f| FileCoverage {
            filename: f.filename,
            summary: f.summary.into(),
        })
        .collect();
    files.sort_by(|a, b| a.filename.cmp(&b.filename));

    Ok(CoverageReport {
        totals: first.totals.into(),
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    const SAMPLE_JSON: &str = r#"{
      "type": "llvm.coverage.json.export",
      "version": "2.0.1",
      "data": [
        {
          "totals": {
            "lines": { "count": 100, "covered": 85, "percent": 85.0 },
            "functions": { "count": 20, "covered": 18, "percent": 90.0 },
            "branches": { "count": 10, "covered": 8, "percent": 80.0 },
            "regions": { "count": 150, "covered": 120, "percent": 80.0 }
          },
          "files": [
            {
              "filename": "src/lib.rs",
              "summary": {
                "lines": { "count": 60, "covered": 57, "percent": 95.0 },
                "functions": { "count": 12, "covered": 12, "percent": 100.0 },
                "regions": { "count": 90, "covered": 84, "percent": 93.33 }
              }
            },
            {
              "filename": "src/error.rs",
              "summary": {
                "lines": { "count": 40, "covered": 28, "percent": 70.0 },
                "functions": { "count": 8, "covered": 6, "percent": 75.0 },
                "regions": { "count": 60, "covered": 36, "percent": 60.0 }
              }
            }
          ]
        }
      ]
    }"#;

    #[googletest::test]
    fn parse_llvm_cov_json_extracts_totals_and_sorted_files()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = parse_llvm_cov_json(SAMPLE_JSON)?;
        expect_that!(report.line_coverage_percent(), eq(85.0));
        expect_that!(report.totals.lines.count, eq(100));
        expect_that!(report.totals.lines.covered, eq(85));
        expect_that!(report.totals.functions.percent, eq(90.0));
        expect_that!(report.totals.branches.percent, eq(80.0));
        expect_that!(report.totals.regions.percent, eq(80.0));

        expect_that!(report.files.len(), eq(2));
        let first = report.files.first().ok_or("missing first file")?;
        expect_that!(&first.filename, eq("src/error.rs"));
        expect_that!(first.summary.lines.percent, eq(70.0));

        let below_80 = report.files_below_threshold(80.0);
        expect_that!(below_80.len(), eq(1));
        Ok(())
    }

    #[googletest::test]
    fn render_console_and_lcov_formats_expected_output() -> Result<(), Box<dyn std::error::Error>> {
        let report = parse_llvm_cov_json(SAMPLE_JSON)?;
        let mut buf = Vec::new();
        report.render_console(&mut buf)?;
        let text = String::from_utf8(buf)?;
        expect_that!(text, contains_substring("src/lib.rs"));
        expect_that!(text, contains_substring("TOTAL"));
        expect_that!(text, contains_substring("85.00%"));

        let lcov = report.format_lcov();
        expect_that!(lcov, contains_substring("SF:src/lib.rs"));
        expect_that!(lcov, contains_substring("LF:60"));
        expect_that!(lcov, contains_substring("LH:57"));
        expect_that!(lcov, contains_substring("end_of_record"));
        Ok(())
    }
}
