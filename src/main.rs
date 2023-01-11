use log::debug;

mod block;
mod db;
mod utils;

const DEFAULT_API: &str = "https://vm.aleo.org/api/testnet3";

#[tokio::main]
async fn main() {
    env_logger::init();
    let api = get_api();
    debug!("use aleo api: {}", api);
    let _mysql_url = String::from("mysql://root:1234567890@127.0.0.1:3306/aleo_blocks");
    // let _sql_client = db::MysqlClient::new(&mysql_url);
    // check 三张表block height的连续性和一致性

    // 获取当前数据库已记录的latest_height
    let latest_height = 372595_u32;
    // 使用latest_height的block, 不断获取next_block，并计算next_block的reward
    block::get_blocks(api, latest_height).await;
}

fn get_api() -> String {
    let mut api = String::new();
    match std::env::var("ALEO_API") {
        Ok(a) => api.push_str(&a),
        Err(_e) => {
            api.push_str(DEFAULT_API);
        }
    }

    api
}

