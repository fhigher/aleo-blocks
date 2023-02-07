use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::str::FromStr;

use clap::Parser;
use anyhow::Result;

use crate::cli::config::load_config;
use crate::storage::Store;
use crate::mysql::MysqlClient;
use crate::server::Server;
use crate::utils::runtime;

use snarkvm_console_network::Testnet3;

/// Api server to query
#[derive(Debug, Parser)]
pub enum Api {
    /// Start api server
    Start {
        #[clap(default_value = "config.yml", long = "config")]
        config: String,
    },
}

impl Api {
    pub fn parse(self) -> Result<String> {
        match self {
            Self::Start{ config } => {
                let config = load_config(config);
                runtime().block_on(async move {
                    #[cfg(feature = "mysql")]
                    let store = Store::<Testnet3, MysqlClient>::new(String::from(config.mysqldsn));
                    let default = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9898);
                    let ip = SocketAddr::from_str(&config.listen_ip).unwrap_or(default);
                    #[cfg(feature = "mysql")]
                    Server::<Testnet3, MysqlClient>::start(ip, store);
                    std::future::pending::<()>().await;
                });
                
                Ok(String::new())
            }
        }
    }
   
}