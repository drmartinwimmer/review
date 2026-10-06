use clap::Parser;
use code_review_api::{ApiCommand, ApiError};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "api",
    about = "Introspect and detect public API drift against API.md",
    version
)]
struct Cli {
    #[command(flatten)]
    cmd: ApiCommand,
}

impl Cli {
    fn run(self) -> ExitCode {
        match self.cmd.run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(ApiError::DriftDetected { .. }) => ExitCode::from(1),
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
