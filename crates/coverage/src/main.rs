use clap::Parser;
use code_review_coverage::{CoverageCommand, CoverageError};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "coverage",
    about = "Run LLVM source-based coverage gates",
    version
)]
struct Cli {
    #[command(flatten)]
    cmd: CoverageCommand,
}

impl Cli {
    fn run(self) -> ExitCode {
        match self.cmd.run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(CoverageError::ThresholdNotMet { .. }) => ExitCode::from(1),
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

    #[googletest::test]
    fn run_cli_missing_path_returns_exit_code_2() {
        let cli = Cli::parse_from([
            "coverage",
            "--path",
            "nonexistent_coverage_dir_404",
            "--quiet",
        ]);
        expect_that!(cli.run(), eq(ExitCode::from(2)));
    }
}
