mod app;
mod cli;
#[cfg(feature = "completions")]
mod completions;
mod config;
mod logger;
mod settings;

use clap::Parser;
use config::Config;
use settings::AppSettings;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();

    #[cfg(feature = "completions")]
    if let Some(shell) = cli.completions {
        completions::generate_completions(shell);
        return Ok(());
    }

    let config = Config::load_merged(cli.config.as_deref());
    let settings = AppSettings::resolve(&cli, &config);
    logger::init(&settings.log_level);
    app::run(settings)
}
