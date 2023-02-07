use clap::Parser;
use anyhow::{bail, Result};
use log::{info, error};
use memmap2::MmapMut;
use tokio::sync::mpsc;


use crate::cli::config::{load_config, Config};
use crate::storage::Store;
use crate::mysql::MysqlClient;
use crate::utils::{runtime, open_file};

use snarkvm_console_network::Testnet3;

/// Sync block server and check or update block height file
#[derive(Debug, Parser)]
pub enum Sync {
    /// Start to sync block
    Start {
        #[clap(default_value = "config.yml", long = "config")]
        config: String,
    },
    /// Check block height file
    Check {
        #[clap(default_value = "block_height.sync", long = "file")]
        file: String,
    },
    /// Update block height file
    Update {
        #[clap(default_value = "block_height.sync", long = "file")]
        file: String,
        #[clap(long = "height")]
        height: u32,
    },
}

impl Sync {
    pub fn parse(self) -> Result<String> {
        match self {
            Self::Start{ config } => {
                let config = load_config(config);
                {
                    let address = &config.address;
                    if address.is_empty() {
                        bail!("must config wallet address...");
                    }
                    info!("sync block data with address only: {:?}", &address);
                }
                runtime().block_on(async move { 
                    Self::sync(config).await;
                });
                
                Ok(String::new())
            },
            Self::Check { file } => {
                let file = open_file(file);
                let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
            
                let mut buf: [u8; 4] = [0, 0, 0, 0];
                buf.copy_from_slice(mmap.get(0..mmap.len()).unwrap());
                let latest_height = u32::from_le_bytes(buf);
                println!("get latest_height {} from file", latest_height);

                Ok(String::new())
            },
            Self::Update { file, height } => {
                let file = open_file(file);
                let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
                let height = u32::to_le_bytes(height);
                mmap.copy_from_slice(&height[..]);

                Ok(String::new())
            }
        }
    }

    async fn sync(config: Config){
        let api = config.aleoapi;
        info!("use aleo api: {:?}", api);

        let address = config.address;
        let file = open_file(config.synced_height_file);
        let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
        // 获取已同步的高度
        let mut buf: [u8; 4] = [0, 0, 0, 0];
        buf.copy_from_slice(mmap.get(0..mmap.len()).unwrap());
        let latest_height = u32::from_le_bytes(buf);
        info!("get sync latest height {} from file", latest_height);

        let client = reqwest::Client::builder().build().unwrap();
        let (sender, receiver) = mpsc::channel(4096);
    
        #[cfg(feature = "mysql")]
        let store = Store::<Testnet3, MysqlClient>::new(String::from(config.mysqldsn));
    
        // 消息处理
        tokio::spawn(async move {
            #[cfg(feature = "mysql")]
            crate::message::handle::<Testnet3, MysqlClient>(store, receiver, mmap).await;
        });

          // 批量同步历史区块
        let batch_obj = crate::batch::Batch::<Testnet3>::new(
            &client, 
            &api[0], 
            latest_height,
            None, 
            &address, 
            config.batch_request, 
            config.batch_concurrent, 
            sender.clone(),
            config.store_block,
        );
        let latest_height = match batch_obj.get_blocks().await {
            Ok(height) => height,
            Err((height, error)) => {
                error!("batch load blocks {}: {}", height, error);
                return;
            }
        };

        // 同步单个区块
        let single_obj = crate::single::Single::<Testnet3>::new(
            api, 
            latest_height, 
            &client, 
            &address, 
            sender.clone(),
            config.store_block,
        );
        if let Err(e) = single_obj.get_blocks().await {
            error!("get_blocks_one_by_one: {:?}", e);
        }
    }
}