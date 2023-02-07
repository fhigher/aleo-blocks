use log::warn;
use clap::Parser;

mod single;
mod batch;
mod parse;
mod mysql;
mod storage;
mod utils;
mod message;
mod server;
mod cli;

fn main() {
    if let Err(e) = std::env::var("RUST_LOG") {
        warn!("the log level env {:?}, set default.", e);
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();
    
    let cli = cli::CLI::parse();

    match cli.command.parse() {
        Ok(output) => println!("{output}\n"),
        Err(error) => println!("⚠️  {error}\n"),
    }
}

