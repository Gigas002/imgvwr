use std::{
    fs,
    path::PathBuf
};
use clap::Parser;
use crate::config::Config;

#[derive(Clone, Debug, Parser, Default)]
pub struct Args {
    #[arg(short, long)]
    pub img: PathBuf,
    #[arg(short, long)]
    config: Option<PathBuf>,
}

impl Args {
    pub fn get_config(&self) -> Option<Config> {
        let mut config_path = self.config.clone().unwrap_or_else(|| {
            let dotconfig = dirs::config_local_dir().expect("Couldn't get .config directory");

            dotconfig.join("imgvwr").join("config.toml")
        });
        if config_path.exists() {
            config_path = fs::canonicalize(&config_path).expect("Couldn't get absolute path for config file");

            Config::load(&config_path)
        }
        else {
            None
        }
    }
}
