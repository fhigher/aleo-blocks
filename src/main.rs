use log::{debug, error};
use tokio::sync::mpsc;

mod single;
mod batch;
mod parse;
mod mysql;
mod storage;
mod config;
mod utils;

use snarkvm_console_network::Testnet3;



#[tokio::main]
async fn main() {
    env_logger::init();
    let config = config::load_config();

    let api = config.aleoapi;
    debug!("use aleo api: {:?}", api);

    let address = config.address;
    debug!("sync block data with address only: {:?}", &address);

    let url = String::from(config.mysqldns);
    #[cfg(feature = "mysql")]
    let store = storage::Store::<Testnet3, mysql::MysqlClient>::new(url);
    // 获取当前数据库已记录的latest_height
    let latest_height = store.latest_height();
    debug!("get latest_height {} from database", latest_height);
    
    let client = reqwest::Client::builder().build().unwrap();
    let (solution_sender, solution_receiver) = mpsc::channel(4096);
    let (block_sender, block_receiver) = mpsc::channel(4096);

    tokio::spawn(async move {
        #[cfg(feature = "mysql")]
        parse::handle::<Testnet3, mysql::MysqlClient>(store, solution_receiver, block_receiver).await;
    });

    let batch_obj = batch::Batch::<Testnet3>::new(
        &client, 
        &api[0], 
        latest_height,
        None, 
        &address, 
        config.batch_request, 
        config.batch_concurrent, 
        solution_sender.clone(),
        block_sender.clone()
    );
    let latest_height = match batch_obj.get_blocks().await {
        Ok(height) => height,
        Err((height, error)) => {
            error!("batch load blocks {}: {}", height, error);
            return;
        }
    };

    // 批量同步完后，继续获取next_block，并计算next_block的reward
    let single_obj = single::Single::<Testnet3>::new(
        api, 
        latest_height, 
        &client, 
        &address, 
        solution_sender.clone(),
        block_sender.clone()
    );
    if let Err(e) = single_obj.get_blocks().await {
        error!("get_blocks_one_by_one: {:?}", e);
    }
}