mod app;
mod cli;
#[cfg(feature = "completions")]
mod completions;
mod config;
mod logger;
mod settings;

use std::process::ExitCode;

use clap::Parser;
use config::Config;
use settings::AppSettings;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();

    #[cfg(feature = "completions")]
    if let Some(shell) = cli.completions {
        completions::generate_completions(shell);
        return ExitCode::SUCCESS;
    }

    let mut problems = Vec::new();
    let config = match Config::load_merged(cli.config.as_deref(), &mut problems) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("imgvwr: {e}");
            return ExitCode::FAILURE;
        }
    };
    let settings = AppSettings::resolve(&cli, &config, &mut problems);
    logger::init(&settings.log_level);
    // Found before logging could be set up from the config.
    for problem in &problems {
        tracing::warn!("{problem}");
    }
    match app::run(settings) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Printed directly: logging may be compiled out or filtered.
            eprintln!("imgvwr: {e}");
            ExitCode::FAILURE
        }
    }
}
