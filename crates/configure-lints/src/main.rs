use clap::Parser;
use code_review_configure_lints::ConfigureLintsOptions;
use code_review_configure_lints::cargo_toml::LintProfile;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "configure-lints",
    about = "Configure or remove strict Clippy lints in Cargo.toml",
    version
)]
struct Cli {
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

impl Cli {
    fn to_options(&self) -> ConfigureLintsOptions {
        ConfigureLintsOptions {
            manifest_path: self.manifest_path.clone(),
            profile: self.profile,
            remove: self.remove,
            quiet: self.quiet,
        }
    }

    fn run(self) -> ExitCode {
        let opts = self.to_options();
        match code_review_configure_lints::run(&opts) {
            Ok(result) => {
                if !opts.quiet {
                    if opts.remove {
                        if result.modified {
                            eprintln!(
                                "Removed {} Clippy lints from '{}'.",
                                result.lints_configured,
                                opts.manifest_path.display()
                            );
                        } else {
                            eprintln!(
                                "No Clippy lints found in '{}'. Manifest unchanged.",
                                opts.manifest_path.display()
                            );
                        }
                    } else if result.modified {
                        eprintln!(
                            "Configured {} Clippy lints ({:?} profile) in '{}'.",
                            result.lints_configured,
                            opts.profile,
                            opts.manifest_path.display()
                        );
                    } else {
                        eprintln!(
                            "Manifest '{}' already configured with {} Clippy lints ({:?} profile). Manifest unchanged.",
                            opts.manifest_path.display(),
                            result.lints_configured,
                            opts.profile
                        );
                    }
                }
                ExitCode::SUCCESS
            }
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
    use code_review_configure_lints::cargo_toml::LintProfile;
    use googletest::prelude::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(name: &str) -> Self {
            let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!("{name}_{}_{count}", std::process::id()));
            drop(fs::create_dir_all(&path));
            Self { path }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.path));
        }
    }

    #[googletest::test]
    fn run_configure_lints_with_valid_manifest_succeeds() -> Result<(), Box<dyn std::error::Error>>
    {
        let guard = TempDirGuard::new("test_configure_lints_bin");
        let manifest_path = guard.path().join("Cargo.toml");
        fs::write(
            &manifest_path,
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )?;

        let cli = Cli {
            manifest_path: manifest_path.clone(),
            profile: LintProfile::Strict,
            remove: false,
            quiet: true,
        };
        let exit_code = cli.run();
        expect_that!(exit_code, eq(ExitCode::SUCCESS));

        let content = fs::read_to_string(&manifest_path)?;
        expect_that!(content, contains_substring("[lints.clippy]"));
        Ok(())
    }

    #[googletest::test]
    fn run_configure_lints_with_missing_manifest_returns_error() {
        let guard = TempDirGuard::new("test_configure_lints_missing");
        let manifest_path = guard.path().join("NonExistent.toml");

        let cli = Cli {
            manifest_path,
            profile: LintProfile::Strict,
            remove: false,
            quiet: true,
        };
        let exit_code = cli.run();
        expect_that!(exit_code, eq(ExitCode::from(2)));
    }
}
