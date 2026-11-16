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

    let config = Config::load_merged(cli.config.as_deref());
    let settings = AppSettings::resolve(&cli, &config);
    logger::init(&settings.log_level);
    match app::run(settings) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Printed directly: logging may be compiled out or filtered.
            eprintln!("imgvwr: {e}");
            ExitCode::FAILURE
        }
    }
}
