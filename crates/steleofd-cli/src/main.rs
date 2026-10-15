//! Process entry point: parse, localize, dispatch, set the exit code.
//! All logic lives in the library so tests can drive it in-process.

use std::process::ExitCode;

use clap::Parser;

use steleofd_cli::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let i18n = steleofd_cli::i18n_for(&cli);
    init_tracing(cli.verbose);

    match steleofd_cli::run(&cli, &i18n) {
        Ok(report) => {
            if !report.text.is_empty() {
                println!("{}", report.text);
            }
            if report.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(err) => {
            eprintln!("{}", steleofd_cli::error_report(&err, &i18n).text);
            ExitCode::from(2)
        }
    }
}

/// Log to stderr, honoring RUST_LOG when set.
fn init_tracing(verbose: bool) {
    let default = if verbose {
        "steleofd=info"
    } else {
        "steleofd=warn"
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}
