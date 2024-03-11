use std::path::PathBuf;
use clap::Parser;
use crate::config::Config;

#[derive(Clone, Debug, Parser, Default)]
pub struct Args {
    #[arg(short, long)]
    pub img: PathBuf,
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

impl Args {
    pub fn get_config(&self) -> Config {
        let config_path = self.config.clone().unwrap_or_else(|| {
            let dotconfig = dirs::config_local_dir().unwrap();

            dotconfig.join("imgvwr").join("config.toml")
        });

        Config::load(config_path)
    }
}
