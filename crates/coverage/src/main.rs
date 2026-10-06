use clap::Parser;
use code_review_coverage::CoverageOptions;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "coverage",
    about = "Run LLVM source-based coverage gates",
    version
)]
struct Cli {
    /// Minimum coverage threshold percentage
    #[arg(long)]
    threshold: Option<f64>,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl Cli {
    fn to_options(&self) -> CoverageOptions {
        CoverageOptions {
            threshold: self.threshold,
            quiet: self.quiet,
        }
    }

    fn run(self) -> ExitCode {
        let opts = self.to_options();
        if !opts.quiet {
            eprintln!("Notice: coverage runner is scheduled for future milestones.");
        }
        if let Err(err) = code_review_coverage::run(&opts) {
            eprintln!("Error: {err}");
            ExitCode::from(2)
        } else {
            ExitCode::SUCCESS
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    cli.run()
}
