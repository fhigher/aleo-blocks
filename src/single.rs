use log::{error, info, warn};
use tokio::time::{sleep, Duration};
use std::io::{self, ErrorKind};
use std::marker::PhantomData;
use tokio::sync::mpsc;
use backoff::future::retry;
use crate::utils::{backoffset, from_reqwest_err};
use crate::message::Message;

use snarkvm_console_network::Network;
use snarkvm_synthesizer::Block;

pub struct Single<'a, N: Network> {
    apis: Vec<String>,
    latest_height: u32,
    client: &'a reqwest::Client,
    address: &'a Vec<String>,
    sender: mpsc::Sender<Message<N>>,
    store_block: bool,
    _n: PhantomData<N>,
}

impl<'a, N:Network> Single<'a, N> {
    pub fn new(
        apis: Vec<String>, 
        latest_height: u32, 
        client: &'a reqwest::Client, 
        address: &'a Vec<String>, 
        sender: mpsc::Sender<Message<N>>,
        store_block: bool,
    ) -> Self {
        Self { 
            apis, 
            latest_height, 
            client, 
            address,
            sender,
            store_block,
            _n: PhantomData}
    }

    pub async fn get_blocks(&self) -> anyhow::Result<()>{
        let mut latest_height_mut = self.latest_height;
        let mut blocks: Option<(u32, Block<N>)> = None;
        let mut chain_height;
        let mut result;
        let api = &self.apis[0];
        let mut error = None;
        loop { 
            if ! error.is_none() {
                // TODO 更换api
                info!("alter api");
            }
            result = self.get_chain_height(api).await;
            match result {
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await?;
                    if status != reqwest::StatusCode::OK {
                        error!("get chain height response status: {}, body: {}", status.as_str(), body);
                        error = Some(io::Error::new(ErrorKind::Other, body));
                        continue;
                    }
                    chain_height = body.parse::<u32>().unwrap();
                    info!("latest chain height: {}", chain_height);
                },
                Err(e) => {
                    error!("get chain height {:?}" , e);
                    error = Some(io::Error::new(ErrorKind::Other, e.to_string()));
                    continue;
                }
            }
            // 链上最新高度 - latest_height_mut < 0, 则更换api并跳过
            if chain_height < latest_height_mut {
                warn!("api {} does not sync to the latest height", api);
                error = Some(io::Error::new(ErrorKind::Other, ""));
                continue;
            }
    
            // 链上最新高度 - latest_height_mut <= 10, 则跳过，避免获取到分叉块
            if chain_height - latest_height_mut <= 10 {
                // sleep 出块时间
                warn!("had recorded latest height: {}, waiting {} new blocks will be confirmed...", latest_height_mut, 10);
                sleep(Duration::from_secs(15)).await;
                continue;
            }
    
            // 批量区块同步完成后，开始单个区块拉取时，获取一次latest_height的区块
            if latest_height_mut == self.latest_height {
                info!("get first block: {}", self.latest_height);
                result = self.get_block(api, self.latest_height).await;
                match result {
                    Ok(response)=> {
                        let status = response.status();
                        let body = response.text().await?;
                        if status != reqwest::StatusCode::OK {
                            error!("get first block response status: {}, body: {}", status.as_str(), body);
                            error = Some(io::Error::new(ErrorKind::Other, body));
                            continue;
                        }
                        let latest_block = serde_json::from_str(&body).unwrap();
                        blocks.replace((self.latest_height, latest_block));
                    },
                    Err(e) => {
                        error!("get first block {}, {:?}", self.latest_height , e);
                        error = Some(io::Error::new(ErrorKind::Other, e.to_string()));
                        continue;
                    }
                }
            }
            
            // 获取current_height 对应的block, 并计算该block奖励
            let current_height = latest_height_mut + 1;  
            info!("get current block: {}", current_height);  
            result = self.get_block(api, current_height).await;
            match result {
                Ok(response )=> {
                    let status = response.status();
                    let body = response.text().await?;
                    if status != reqwest::StatusCode::OK {
                        error!("get current block response status: {}, body: {}", status.as_str(), body);
                        error = Some(io::Error::new(ErrorKind::Other, body));
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
                    error = Some(io::Error::new(ErrorKind::Other, e.to_string()));
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

    async fn get_block(&self, api: &String, height: u32) 
        -> Result<reqwest::Response, reqwest::Error> {
        retry(backoffset(), || async {
            let block_api = format!("{}/block/{}", api, height);
            self.client.get(block_api).send().await.map_err(from_reqwest_err) 
        }).await
    }

    async fn get_chain_height(&self, api: &String) 
        -> Result<reqwest::Response, reqwest::Error> {
        retry(backoffset(), || async {
            let height_api = format!("{}/latest/height", api);
            self.client.get(height_api).send().await.map_err(from_reqwest_err) 
        }).await
    }
}












