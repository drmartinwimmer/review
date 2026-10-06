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

    /// Run cargo fmt checks (enabled by default; set to false to skip)
    #[arg(
        long,
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args(0..=1),
        default_missing_value = "true"
    )]
    fmt: bool,

    /// Run cargo clippy checks (enabled by default; set to false to skip)
    #[arg(
        long,
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args(0..=1),
        default_missing_value = "true"
    )]
    clippy: bool,

    /// Run purist AST linter checks (enabled by default; set to false to skip)
    #[arg(
        long,
        alias = "opinionated",
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args(0..=1),
        default_missing_value = "true"
    )]
    purist: bool,

    /// Run cargo audit dependency security scan (enabled by default; set to false to skip)
    #[arg(
        long,
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args(0..=1),
        default_missing_value = "true"
    )]
    audit: bool,

    /// Run markdown format/lint checks (enabled by default; set to false to skip)
    #[arg(
        long,
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args(0..=1),
        default_missing_value = "true"
    )]
    markdown: bool,

    /// Run TOML format/lint checks (enabled by default; set to false to skip)
    #[arg(
        long,
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args(0..=1),
        default_missing_value = "true"
    )]
    toml: bool,

    /// Run JSON format/lint checks (enabled by default; set to false to skip)
    #[arg(
        long,
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args(0..=1),
        default_missing_value = "true"
    )]
    json: bool,

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
            fmt: self.fmt,
            clippy: self.clippy,
            purist: self.purist,
            audit: self.audit,
            markdown: self.markdown,
            toml: self.toml,
            json: self.json,
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
            fmt: false,
            clippy: false,
            purist: false,
            audit: false,
            markdown: false,
            toml: false,
            json: false,
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
            fmt: false,
            clippy: false,
            purist: false,
            audit: false,
            markdown: false,
            toml: false,
            json: false,
            quiet: true,
        };
        assert_that!(cli.run(), eq(ExitCode::from(2)));
    }
}
