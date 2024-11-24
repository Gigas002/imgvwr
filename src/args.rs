use clap::Parser;
use std::path::PathBuf;

#[derive(Clone, Debug, Parser, Default)]
pub struct Args {
    pub img: PathBuf,

    #[arg(short, long)]
    pub config: Option<PathBuf>,
}
