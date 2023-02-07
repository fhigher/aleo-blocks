use clap::Parser;
use anyhow::Result;
use clap::AppSettings::ColoredHelp;

mod sync;
mod api;
mod config;

#[derive(Debug, Parser)]
#[clap(name = "aleo-tools", author = "https://github.com/labs3", setting = ColoredHelp)]
pub struct CLI {
    /// Specify a subcommand
    #[clap(subcommand)]
    pub command: Command
}

#[derive(Debug, Parser)]
pub enum Command {
    #[clap(subcommand)]
    Sync(sync::Sync),
    #[clap(subcommand)]
    Server(api::Api),
}

impl Command {
    pub fn parse(self) -> Result<String> {
        match self {
            Self::Sync(command) => command.parse(),
            Self::Server(command) => command.parse(),
        }
    }
}
