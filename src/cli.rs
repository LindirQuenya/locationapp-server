use std::path::PathBuf;

use clap::Parser;
// TODO: clap
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    /// Config file
    #[arg(short, long, value_name = "FILE")]
    pub(crate) config: PathBuf,
}
