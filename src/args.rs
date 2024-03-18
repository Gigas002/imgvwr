use std::{
    fs,
    path::PathBuf
};
use clap::Parser;
use crate::{
    config::Config,
    strings::{
        self,
        messages
    },
};

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
            let dotconfig = dirs::config_local_dir().expect(messages::ERR_NO_DOTCONFIG);

            dotconfig.join(strings::APPLICATION_NAME).join(strings::CONFIG_FILENAME)
        });
        config_path.exists().then(||{
            config_path = fs::canonicalize(&config_path).expect(messages::ERR_NO_ABS_PATH_CONFIG);

            Config::load(&config_path).unwrap_or_default()
        })
    }
}
