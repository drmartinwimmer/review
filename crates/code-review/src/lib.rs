use clap::{Parser, Subcommand};
use code_review_api::{ApiCommand, ApiError};
use code_review_check::{self, CheckError, CheckOptions, FailOn};
use code_review_configure_lints::{self, CargoTomlError, ConfigureLintsOptions, LintProfile};
use code_review_coverage::{self, CoverageError, CoverageOptions};
use purist::{self, OutputFormat, PuristError, PuristOptions};
use std::path::PathBuf;
use std::process::ExitCode;
use thiserror::Error;

/// Error type for the code-review CLI toolkit and subcommands.
#[derive(Debug, Error)]
pub enum CodeReviewError {
    /// Errors originating from the configure-lints subcommand.
    #[error(transparent)]
    ConfigureLints(#[from] CargoTomlError),

    /// Errors originating from the check aggregator subcommand.
    #[error(transparent)]
    Check(#[from] CheckError),

    /// Errors originating from the purist linter subcommand.
    #[error(transparent)]
    Purist(#[from] PuristError),

    /// Errors originating from the API drift detector subcommand.
    #[error(transparent)]
    Api(#[from] ApiError),

    /// Errors originating from the coverage runner subcommand.
    #[error(transparent)]
    Coverage(#[from] CoverageError),
}

/// Arguments for the check aggregator subcommand.
#[derive(clap::Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckArgs {
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

impl CheckArgs {
    /// Converts arguments to domain `CheckOptions`.
    pub fn into_options(self, fallback_format: OutputFormat) -> CheckOptions {
        CheckOptions {
            path: self.path,
            format: Some(self.format.unwrap_or(fallback_format)),
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

    /// Executes the check subcommand.
    pub fn run(self, format: OutputFormat) -> Result<(), CheckError> {
        let options = self.into_options(format);
        code_review_check::run(&options)
    }
}

/// Arguments for the configure-lints subcommand.
#[derive(clap::Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigureLintsArgs {
    /// Path to the Cargo.toml manifest to configure
    #[arg(long, default_value = "Cargo.toml")]
    manifest_path: PathBuf,

    /// Lint profile preset to inject (strict or standard)
    #[arg(long, value_enum, default_value_t = LintProfile::Strict)]
    profile: LintProfile,

    /// Remove configured lints instead of injecting them
    #[arg(long)]
    remove: bool,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl ConfigureLintsArgs {
    /// Converts arguments to domain `ConfigureLintsOptions`.
    pub fn into_options(self) -> ConfigureLintsOptions {
        ConfigureLintsOptions {
            manifest_path: self.manifest_path,
            profile: self.profile,
            remove: self.remove,
            quiet: self.quiet,
        }
    }

    /// Executes the configure-lints subcommand.
    pub fn run(self) -> Result<(), CargoTomlError> {
        let options = self.into_options();
        let _ = code_review_configure_lints::run(&options)?;
        Ok(())
    }
}

/// Arguments for the purist linter subcommand.
#[derive(clap::Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct PuristArgs {
    /// Path to source files or crate directory
    #[arg(long)]
    path: Option<PathBuf>,

    /// Output format for reports and diagnostics
    #[arg(long, value_enum)]
    format: Option<OutputFormat>,

    /// Automatically apply fixes where supported (stub)
    #[arg(long)]
    fix: bool,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl PuristArgs {
    /// Converts arguments to domain `PuristOptions`.
    pub fn into_options(self, fallback_format: OutputFormat) -> PuristOptions {
        PuristOptions {
            path: self.path,
            format: Some(self.format.unwrap_or(fallback_format)),
            fix: self.fix,
            quiet: self.quiet,
        }
    }

    /// Executes the purist subcommand.
    pub fn run(self, format: OutputFormat) -> Result<(), PuristError> {
        let options = self.into_options(format);
        purist::run(&options)
    }
}

/// Arguments for the coverage subcommand.
#[derive(clap::Args, Debug, Clone, Default, PartialEq)]
pub struct CoverageArgs {
    /// Minimum coverage threshold percentage
    #[arg(long)]
    threshold: Option<f64>,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl CoverageArgs {
    /// Converts arguments to domain `CoverageOptions`.
    pub fn into_options(self) -> CoverageOptions {
        CoverageOptions {
            threshold: self.threshold,
            quiet: self.quiet,
        }
    }

    /// Executes the coverage subcommand.
    pub fn run(self) -> Result<(), CoverageError> {
        let options = self.into_options();
        code_review_coverage::run(&options)
    }
}

/// Subcommands supported by the code-review CLI toolkit.
#[derive(Debug, Subcommand, PartialEq)]
pub enum Commands {
    /// Aggregates formatters, clippy, purist, audit, and coverage checks
    Check(CheckArgs),

    /// Configure or remove strict Clippy lints in Cargo.toml
    #[command(name = "configure-lints")]
    ConfigureLints(ConfigureLintsArgs),

    /// Run AST-based purist linter rules
    #[command(alias = "opinionated")]
    Purist(PuristArgs),

    /// Introspect and detect public API drift against API.md
    Api(ApiCommand),

    /// Run LLVM source-based coverage gates
    Coverage(CoverageArgs),
}

impl Commands {
    /// Executes the subcommand with default formatting.
    pub fn run(self) -> Result<(), CodeReviewError> {
        self.run_with_format(OutputFormat::Console)
    }

    /// Executes the subcommand with the specified report output format.
    pub fn run_with_format(self, format: OutputFormat) -> Result<(), CodeReviewError> {
        match self {
            Self::ConfigureLints(args) => Ok(args.run()?),
            Self::Check(args) => Ok(args.run(format)?),
            Self::Purist(args) => Ok(args.run(format)?),
            Self::Api(cmd) => Ok(cmd.run()?),
            Self::Coverage(args) => Ok(args.run()?),
        }
    }
}

/// Top-level CLI parser for code-review.
#[derive(Debug, Parser, PartialEq)]
#[command(
    name = "code-review",
    about = "Automated code review, static analysis, and lint configuration toolkit",
    version
)]
pub struct Cli {
    /// Output format for reports and diagnostics
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Console)]
    format: OutputFormat,

    /// Increase verbosity level (-v, -vv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Silence non-essential logging output
    #[arg(short, long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

impl Cli {
    /// Creates a new `Cli` instance.
    pub fn new(format: OutputFormat, verbose: u8, quiet: bool, command: Commands) -> Self {
        Self {
            format,
            verbose,
            quiet,
            command,
        }
    }

    /// Returns the configured output format.
    pub fn format(&self) -> OutputFormat {
        self.format
    }

    /// Returns the verbosity level.
    pub fn verbose(&self) -> u8 {
        self.verbose
    }

    /// Returns whether logging output is suppressed.
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Returns a reference to the selected subcommand.
    pub fn command(&self) -> &Commands {
        &self.command
    }

    /// Runs the selected subcommand and returns an exit code.
    pub fn run(self) -> ExitCode {
        match self.command.run_with_format(self.format) {
            Ok(()) => ExitCode::SUCCESS,
            Err(CodeReviewError::Check(CheckError::ViolationsFound { .. }))
            | Err(CodeReviewError::Purist(PuristError::LintViolationsFound { .. })) => {
                ExitCode::from(1)
            }
            Err(err) => {
                eprintln!("Error: {err}");
                ExitCode::from(2)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use code_review_configure_lints::LintProfile;
    use googletest::prelude::*;
    use std::path::{Path, PathBuf};

    #[googletest::test]
    fn parse_cli_default_global_flags_sets_console_format_and_zero_verbosity()
    -> Result<(), Box<dyn std::error::Error>> {
        let args = ["code-review", "check"];
        let cli = Cli::try_parse_from(args)?;
        expect_that!(cli.format(), eq(OutputFormat::Console));
        expect_that!(cli.verbose(), eq(0));
        expect_that!(cli.is_quiet(), is_false());
        let expected = CheckArgs {
            format: Some(OutputFormat::Console),
            ..Default::default()
        };
        expect_that!(cli.command(), eq(&Commands::Check(expected)));
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_global_format_flag_sets_json_and_markdown()
    -> Result<(), Box<dyn std::error::Error>> {
        let args_json = ["code-review", "--format", "json", "check"];
        let cli_json = Cli::try_parse_from(args_json)?;
        expect_that!(cli_json.format(), eq(OutputFormat::Json));

        let args_md = ["code-review", "--format", "markdown", "check"];
        let cli_md = Cli::try_parse_from(args_md)?;
        expect_that!(cli_md.format(), eq(OutputFormat::Markdown));
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_configure_lints_defaults_uses_strict_profile_and_default_manifest()
    -> Result<(), Box<dyn std::error::Error>> {
        let args = ["code-review", "configure-lints"];
        let cli = Cli::try_parse_from(args)?;
        match cli.command() {
            Commands::ConfigureLints(args) => {
                let opts = args.clone().into_options();
                expect_that!(&opts.manifest_path, eq(&PathBuf::from("Cargo.toml")));
                expect_that!(opts.profile, eq(LintProfile::Strict));
                expect_that!(opts.remove, is_false());
            }
            _ => return Err("Expected ConfigureLints subcommand".into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_configure_lints_custom_flags_parses_arguments()
    -> Result<(), Box<dyn std::error::Error>> {
        let args = [
            "code-review",
            "configure-lints",
            "--manifest-path",
            "crates/demo/Cargo.toml",
            "--profile",
            "standard",
            "--remove",
            "--quiet",
        ];
        let cli = Cli::try_parse_from(args)?;
        match cli.command() {
            Commands::ConfigureLints(args) => {
                let opts = args.clone().into_options();
                expect_that!(
                    &opts.manifest_path,
                    eq(&PathBuf::from("crates/demo/Cargo.toml"))
                );
                expect_that!(opts.profile, eq(LintProfile::Standard));
                expect_that!(opts.remove, is_true());
                expect_that!(opts.quiet, is_true());
            }
            _ => return Err("Expected ConfigureLints subcommand".into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_subcommands_dispatches_all_variants() -> Result<(), Box<dyn std::error::Error>> {
        let variants = [
            (["code-review", "check"].as_slice(), "check"),
            (
                ["code-review", "configure-lints"].as_slice(),
                "configure-lints",
            ),
            (["code-review", "purist"].as_slice(), "purist"),
            (["code-review", "opinionated"].as_slice(), "purist"),
            (["code-review", "api"].as_slice(), "api"),
            (["code-review", "coverage"].as_slice(), "coverage"),
        ];

        for (args, expected_name) in variants {
            let cli = Cli::try_parse_from(args)?;
            let actual_name = match cli.command() {
                Commands::Check(_) => "check",
                Commands::ConfigureLints(_) => "configure-lints",
                Commands::Purist(_) => "purist",
                Commands::Api(_) => "api",
                Commands::Coverage(_) => "coverage",
            };
            expect_that!(actual_name, eq(expected_name));
        }
        Ok(())
    }

    #[googletest::test]
    fn run_cli_check_command_returns_success() {
        let args = CheckArgs {
            path: None,
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
        let cli = Cli::new(OutputFormat::Console, 0, true, Commands::Check(args));
        expect_that!(cli.run(), eq(ExitCode::SUCCESS));
    }

    #[googletest::test]
    fn parse_cli_check_subcommand_parses_all_flags() -> Result<(), Box<dyn std::error::Error>> {
        let args = [
            "code-review",
            "--format",
            "json",
            "check",
            "--path",
            "crates/check",
            "--fail-on",
            "errors",
            "--changed-only",
            "--skip-fmt",
            "--skip-clippy",
            "--skip-purist",
            "--skip-audit",
            "--skip-markdown",
            "--skip-toml",
            "--skip-json",
            "--quiet",
        ];
        let cli = Cli::try_parse_from(args)?;
        expect_that!(cli.format(), eq(OutputFormat::Json));
        match cli.command() {
            Commands::Check(args) => {
                let opts = args.clone().into_options(OutputFormat::Json);
                expect_that!(
                    &opts.path,
                    eq(&Some(Path::new("crates/check").to_path_buf()))
                );
                expect_that!(opts.fail_on, eq(FailOn::Errors));
                expect_that!(opts.changed_only, is_true());
                expect_that!(opts.skip_fmt, is_true());
                expect_that!(opts.skip_clippy, is_true());
                expect_that!(opts.skip_purist, is_true());
                expect_that!(opts.skip_audit, is_true());
                expect_that!(opts.skip_markdown, is_true());
                expect_that!(opts.skip_toml, is_true());
                expect_that!(opts.skip_json, is_true());
                expect_that!(opts.quiet, is_true());
            }
            _ => return Err("Expected Check subcommand".into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_check_subcommand_supports_skip_opinionated_alias()
    -> Result<(), Box<dyn std::error::Error>> {
        let args = ["code-review", "check", "--skip-opinionated"];
        let cli = Cli::try_parse_from(args)?;
        match cli.command() {
            Commands::Check(args) => {
                let opts = args.clone().into_options(OutputFormat::Console);
                expect_that!(opts.skip_purist, is_true());
            }
            _ => return Err("Expected Check subcommand".into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_purist_subcommand_parses_flags() -> Result<(), Box<dyn std::error::Error>> {
        let args = [
            "code-review",
            "--format",
            "json",
            "purist",
            "--path",
            "crates/purist/src",
            "--fix",
            "--quiet",
        ];
        let cli = Cli::try_parse_from(args)?;
        expect_that!(cli.format(), eq(OutputFormat::Json));
        match cli.command() {
            Commands::Purist(args) => {
                let opts = args.clone().into_options(OutputFormat::Json);
                expect_that!(
                    &opts.path,
                    eq(&Some(
                        std::path::Path::new("crates/purist/src").to_path_buf()
                    ))
                );
                expect_that!(opts.fix, is_true());
                expect_that!(opts.quiet, is_true());
            }
            _ => return Err("Expected Purist subcommand".into()),
        }
        Ok(())
    }

    struct TempDirGuard(std::path::PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(std::fs::remove_dir_all(&self.0));
        }
    }

    #[googletest::test]
    fn run_cli_purist_command_on_clean_target_returns_success()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_code_review_purist_clean_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("clean.rs");
        std::fs::write(&file_path, "pub fn helper() -> i32 { 10 }\n")?;

        let args = PuristArgs {
            path: Some(file_path),
            format: None,
            fix: false,
            quiet: true,
        };
        let cli = Cli::new(OutputFormat::Console, 0, true, Commands::Purist(args));
        let code = cli.run();

        expect_that!(code, eq(ExitCode::SUCCESS));
        Ok(())
    }

    #[googletest::test]
    fn run_cli_purist_command_with_violations_returns_exit_code_1()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_code_review_purist_viol_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("main.rs");
        std::fs::write(&file_path, "mod helpers { pub fn broken() {} }\n")?;

        let args = PuristArgs {
            path: Some(file_path),
            format: None,
            fix: false,
            quiet: true,
        };
        let cli = Cli::new(OutputFormat::Console, 0, true, Commands::Purist(args));
        let code = cli.run();

        expect_that!(code, eq(ExitCode::from(1)));
        Ok(())
    }
}
