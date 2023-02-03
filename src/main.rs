use log::{debug, error, warn};
use tokio::sync::mpsc;
use std::fs::OpenOptions;
use std::path::PathBuf;
use memmap2::MmapMut;

mod single;
mod batch;
mod parse;
mod mysql;
mod storage;
mod config;
mod utils;
mod message;

use snarkvm_console_network::Testnet3;

#[tokio::main]
async fn main() {
    if let Err(e) = std::env::var("RUST_LOG") {
        warn!("the log level env {:?}, set default.", e);
        std::env::set_var("RUST_LOG", "debug");
    }
    env_logger::init();
    let config = config::load_config();

    let api = config.aleoapi;
    debug!("use aleo api: {:?}", api);

    let address = config.address;
    if address.is_empty() {
        error!("must specify wallet address...");
        return;
    }
    debug!("sync block data with address only: {:?}", &address);

    let path: PathBuf = PathBuf::from("block_height.sync");
    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&path)
                        .unwrap();
    file.set_len(4).unwrap();
    let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    // 获取已同步的高度
    let mut buf: [u8; 4] = [0, 0, 0, 0];
    buf.copy_from_slice(mmap.get(0..mmap.len()).unwrap());
    let latest_height = u32::from_le_bytes(buf);
    debug!("get sync latest height {} from file", latest_height);
    
    let client = reqwest::Client::builder().build().unwrap();
    let (sender, receiver) = mpsc::channel(4096);

    #[cfg(feature = "mysql")]
    let store = storage::Store::<Testnet3, mysql::MysqlClient>::new(String::from(config.mysqldsn));

    tokio::spawn(async move {
        #[cfg(feature = "mysql")]
        message::handle::<Testnet3, mysql::MysqlClient>(store, receiver, mmap).await;
    });

    // 批量同步历史区块
    let batch_obj = batch::Batch::<Testnet3>::new(
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
    let single_obj = single::Single::<Testnet3>::new(
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

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::path::PathBuf;
    use memmap2::MmapMut;

    #[tokio::test]
    async fn test_mmap() {
        let path: PathBuf = PathBuf::from("block_height.sync");
        let file = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create(true)
                            .open(&path)
                            .unwrap();
        file.set_len(4).unwrap();
        let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    
        let mut buf: [u8; 4] = [0, 0, 0, 0];
        buf.copy_from_slice(mmap.get(0..mmap.len()).unwrap());
        let latest_height = u32::from_le_bytes(buf);
        println!("get latest_height {} from file", latest_height);

        let height = u32::to_le_bytes(6975);
        mmap.copy_from_slice(&height[..]);
    }
}