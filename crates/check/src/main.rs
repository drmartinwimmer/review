use clap::Parser;
use code_review_check::{CheckError, CheckOptions, FailOn};
use purist::OutputFormat;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "check",
    about = "Aggregates formatters, clippy, purist, audit, and coverage checks",
    version
)]
struct Cli {
    /// Path to target workspace or crate directory
    #[arg(long)]
    path: Option<PathBuf>,

    /// Output format for reports and diagnostics
    #[arg(long, value_enum)]
    format: Option<OutputFormat>,

    /// Severity threshold triggering non-zero exit code
    #[arg(long, value_enum, default_value_t = FailOn::Warnings)]
    fail_on: FailOn,

    /// Filter diagnostics to only files modified in Jujutsu working copy
    #[arg(long)]
    changed_only: bool,

    /// Skip running cargo fmt
    #[arg(long)]
    skip_fmt: bool,

    /// Skip running cargo clippy
    #[arg(long)]
    skip_clippy: bool,

    /// Skip running purist AST linter
    #[arg(long, alias = "skip-opinionated")]
    skip_purist: bool,

    /// Skip running cargo audit
    #[arg(long)]
    skip_audit: bool,

    /// Skip running markdown format/lint checks
    #[arg(long)]
    skip_markdown: bool,

    /// Skip running TOML format/lint checks
    #[arg(long)]
    skip_toml: bool,

    /// Skip running JSON format/lint checks
    #[arg(long)]
    skip_json: bool,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl Cli {
    fn to_options(&self) -> CheckOptions {
        CheckOptions {
            path: self.path.clone(),
            format: self.format,
            fail_on: self.fail_on,
            changed_only: self.changed_only,
            skip_fmt: self.skip_fmt,
            skip_clippy: self.skip_clippy,
            skip_purist: self.skip_purist,
            skip_audit: self.skip_audit,
            skip_markdown: self.skip_markdown,
            skip_toml: self.skip_toml,
            skip_json: self.skip_json,
            quiet: self.quiet,
        }
    }

    fn run(self) -> ExitCode {
        let opts = self.to_options();
        match code_review_check::run(&opts) {
            Ok(()) => ExitCode::SUCCESS,
            Err(CheckError::ViolationsFound { .. }) => ExitCode::from(1),
            Err(err) => {
                eprintln!("Error: {err}");
                ExitCode::from(2)
            }
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    cli.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::fs;
    use std::path::PathBuf;

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    #[googletest::test]
    fn run_cli_clean_target_returns_success() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_check_cli_clean_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        let cli = Cli {
            path: Some(temp_dir),
            format: None,
            fail_on: FailOn::Warnings,
            changed_only: false,
            skip_fmt: true,
            skip_clippy: true,
            skip_purist: true,
            skip_audit: true,
            skip_markdown: true,
            skip_toml: true,
            skip_json: true,
            quiet: true,
        };

        assert_that!(cli.run(), eq(ExitCode::SUCCESS));
        Ok(())
    }

    #[googletest::test]
    fn run_cli_missing_path_returns_exit_code_2() {
        let cli = Cli {
            path: Some(PathBuf::from("nonexistent_path_8888")),
            format: None,
            fail_on: FailOn::Warnings,
            changed_only: false,
            skip_fmt: false,
            skip_clippy: false,
            skip_purist: false,
            skip_audit: false,
            skip_markdown: false,
            skip_toml: false,
            skip_json: false,
            quiet: true,
        };
        assert_that!(cli.run(), eq(ExitCode::from(2)));
    }
}
