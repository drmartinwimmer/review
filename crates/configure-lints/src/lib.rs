pub mod cargo_toml;

pub use cargo_toml::{CargoTomlError, ConfigureResult, LintProfile, configure_lints, remove_lints};
use std::path::PathBuf;

/// Command-line arguments for configuring or removing Clippy lints in Cargo.toml.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigureLintsOptions {
    /// Path to the Cargo.toml manifest to configure
    pub manifest_path: PathBuf,

    /// Lint profile preset to inject (strict or standard)
    pub profile: LintProfile,

    /// Remove configured lints instead of injecting them
    pub remove: bool,

    /// Silence non-essential logging output
    pub quiet: bool,
}

impl ConfigureLintsOptions {
    /// Creates a new `ConfigureLintsOptions` instance.
    pub fn new(
        manifest_path: impl Into<PathBuf>,
        profile: LintProfile,
        remove: bool,
        quiet: bool,
    ) -> Self {
        Self {
            manifest_path: manifest_path.into(),
            profile,
            remove,
            quiet,
        }
    }
}

/// Executes the configuration or removal of Clippy lints in the target manifest according to options.
pub fn run(options: &ConfigureLintsOptions) -> Result<ConfigureResult, CargoTomlError> {
    if options.remove {
        remove_lints(&options.manifest_path)
    } else {
        configure_lints(&options.manifest_path, options.profile)
    }
}
