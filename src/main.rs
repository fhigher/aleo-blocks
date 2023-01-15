use log::{debug, error};

mod single;
mod batch;
mod parse;
mod db;
mod config;
mod utils;

// use snarkos_node_cdn::load_blocks;
use snarkvm_console_network::Testnet3;
use snarkvm_synthesizer::Block;

#[tokio::main]
async fn main() {
    env_logger::init();
    let config = config::load_config();
    let api = config.aleoapi;
    debug!("use aleo api: {:?}", api);
    let mysql_url = String::from(config.mysqldns);
    //let sql_client = db::MysqlClient::new(&mysql_url);
    // check 三张表block height的连续性和一致性

    // 获取当前数据库已记录的latest_height
    let latest_height = 402105_u32;
    let client = reqwest::Client::builder().build().unwrap();

    if let Err((_, error)) = batch::get_blocks(&client, &api[0], 
            latest_height, None,  move |block: Block<Testnet3>, start_height: u32| batch::process_block(&block, start_height)).await {
        error!("batch load blocks: {}", error);
        return;
    }

    // 使用latest_height的block, 不断获取next_block，并计算next_block的reward
    if let Err(e) = single::get_blocks(api, latest_height, &client).await {
        error!("get_blocks_one_by_one: {:?}", e);
    }
}

