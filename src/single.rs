use log::{error, info, warn};
use tokio::time::{sleep, Duration};
use std::marker::PhantomData;
use tokio::sync::mpsc;
use crate::message::Message;
use crate::manage::ApiManage;

use snarkvm_console_network::Network;
use snarkvm_synthesizer::Block;

pub struct Single<'a, N: Network> {
    api_manage: ApiManage,
    latest_height: u32,
    address: &'a Vec<String>,
    sender: mpsc::Sender<Message<N>>,
    store_block: bool,
    _n: PhantomData<N>,
}

impl<'a, N:Network> Single<'a, N> {
    pub fn new(
        api_manage: ApiManage,
        latest_height: u32, 
        address: &'a Vec<String>, 
        sender: mpsc::Sender<Message<N>>,
        store_block: bool,
    ) -> Self {
        Self { 
            api_manage,
            latest_height, 
            address,
            sender,
            store_block,
            _n: PhantomData}
    }

    pub async fn get_blocks(&self) -> anyhow::Result<()>{
        let mut latest_height_mut = self.latest_height;
        let mut blocks: Option<(u32, Block<N>)> = None;
        let mut chain_height;
        let block_duration = 15;
        let mut result;
        loop { 
            result = self.api_manage.get("/latest/height").await;
            match result {
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await?;
                    if status != reqwest::StatusCode::OK {
                        error!("get latest chain height response status: {}, body: {}", status.as_str(), body);
                        sleep(Duration::from_secs(block_duration)).await;
                        continue;
                    }
                    chain_height = body.parse::<u32>().unwrap();
                    info!("get latest chain height {} from api", chain_height);
                },
                Err(e) => {
                    error!("get latest chain height {:?}" , e);
                    sleep(Duration::from_secs(block_duration)).await;
                    continue;
                }
            }
            // 链上最新高度 - latest_height_mut < 0
            if chain_height < latest_height_mut {
                warn!("api does not sync to the latest height");
                sleep(Duration::from_secs(block_duration)).await;
                continue;
            }
    
            // 链上最新高度 - latest_height_mut <= 10, 则跳过，避免获取到分叉块
            if chain_height - latest_height_mut <= 10 {
                sleep(Duration::from_secs(block_duration)).await;
                continue;
            }
    
            // 批量区块同步完成后，开始单个区块拉取时，获取一次latest_height的区块
            if latest_height_mut == self.latest_height {
                info!("get first block: {}", self.latest_height);
                result = self.api_manage.get(format!("/block/{}", self.latest_height).as_str()).await;
                match result {
                    Ok(response)=> {
                        let status = response.status();
                        let body = response.text().await?;
                        if status != reqwest::StatusCode::OK {
                            error!("get first block response status: {}, body: {}", status.as_str(), body);
                            sleep(Duration::from_secs(block_duration)).await;
                            continue;
                        }
                        let latest_block = serde_json::from_str(&body).unwrap();
                        blocks.replace((self.latest_height, latest_block));
                    },
                    Err(e) => {
                        error!("get first block {}, {:?}", self.latest_height , e);
                        sleep(Duration::from_secs(block_duration)).await;
                        continue;
                    }
                }
            }
            
            // 获取current_height 对应的block, 并计算该block奖励
            let current_height = latest_height_mut + 1;  
            info!("get current block: {}", current_height);  
            result = self.api_manage.get(format!("/block/{}", current_height).as_str()).await;
            match result {
                Ok(response )=> {
                    let status = response.status();
                    let body = response.text().await?;
                    if status != reqwest::StatusCode::OK {
                        error!("get current block response status: {}, body: {}", status.as_str(), body);
                        sleep(Duration::from_secs(block_duration)).await;
                        continue;
                    }
                    let current_block: Block<N> = serde_json::from_str(&body).unwrap();
                    let latest_block = blocks.take().unwrap();
                    crate::parse::parse_block::<N>(
                        &current_block, 
                        &latest_block.1, 
                        self.address, 
                        self.sender.clone(),
                        self.store_block,
                    ).await?;
                    latest_height_mut = current_height;
                    blocks.replace((current_height, current_block));
                },
                Err(e) => {
                    error!("get current block {}, {:?}", current_height, e);
                    sleep(Duration::from_secs(block_duration)).await;
                    continue;
                }
            }
        }
         
    }

    /* async fn get_block_once(&self, api: &String, height: u32) 
        -> Result<String, reqwest::Error> {
        let api = format!("{}/block/{}", api, height);
        let res = self.client.get(api)
        .send()
        .await?;
        Ok(res.text().await?)
    } */
}












