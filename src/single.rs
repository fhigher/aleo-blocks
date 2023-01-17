use log::{error, debug};
use tokio::time::{sleep, Duration};
use std::io::{self, ErrorKind};
use std::marker::PhantomData;
use tokio::sync::mpsc;
use backoff::future::retry;
use crate::utils::{backoffset, from_reqwest_err};
use crate::parse::{Solution, BlockReward};

use snarkvm_console_network::Network;
use snarkvm_synthesizer::Block;

pub struct Single<'a, N: Network> {
    apis: Vec<String>,
    latest_height: u32,
    client: &'a reqwest::Client,
    address: &'a Vec<String>,
    solution_sender: mpsc::Sender<Solution<N>>,
    block_sender: mpsc::Sender<BlockReward<N>>,
    _n: PhantomData<N>,
}

impl<'a, N:Network> Single<'a, N> {
    pub fn new(
        apis: Vec<String>, 
        latest_height: u32, 
        client: &'a reqwest::Client, 
        address: &'a Vec<String>, 
        solution_sender: mpsc::Sender<Solution<N>>,
        block_sender: mpsc::Sender<BlockReward<N>>,
    ) -> Self {
        Self { 
            apis, 
            latest_height, 
            client, 
            address,
            solution_sender, 
            block_sender,
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
            // 等待批量同步完成，开始sync one by one
            if ! error.is_none() {
                // TODO 更换api
                debug!("alter api");
            }
            result = self.get_chain_height(api).await;
            match result {
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await?;
                    if status != reqwest::StatusCode::OK {
                        error!("get_chain_height response status: {}, body: {}", status.as_str(), body);
                        error = Some(io::Error::new(ErrorKind::Other, body));
                        continue;
                    }
                    chain_height = body.parse::<u32>().unwrap();
                    debug!("latest chain height: {}", chain_height);
                },
                Err(e) => {
                    error!("get chain height {:?}" , e);
                    error = Some(io::Error::new(ErrorKind::Other, e.to_string()));
                    continue;
                }
            }
            // 链上最新高度 - latest_height_mut < 0, 则更换api并跳过
            if chain_height < latest_height_mut {
                debug!("api {} does not sync to the latest height", api);
                error = Some(io::Error::new(ErrorKind::Other, ""));
                continue;
            }
    
            // 链上最新高度 - latest_height_mut <= 10, 则跳过，避免获取到分叉块
            if chain_height - latest_height_mut <= 10 {
                // sleep 出块时间
                debug!("had recorded latest height: {}, waiting {} new blocks will be confirmed...", latest_height_mut, 10);
                sleep(Duration::from_secs(15)).await;
                continue;
            }
    
            // 进程刚启动执行一次，从数据库拿取的latest_height对应的block
            if latest_height_mut == self.latest_height {
                debug!("get first block: {}", self.latest_height);
                result = self.get_block(api, self.latest_height).await;
                match result {
                    Ok(response)=> {
                        let status = response.status();
                        let body = response.text().await?;
                        if status != reqwest::StatusCode::OK {
                            error!("get_first_block response status: {}, body: {}", status.as_str(), body);
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
            debug!("get current block: {}", current_height);  
            result = self.get_block(api, current_height).await;
            match result {
                Ok(response )=> {
                    let status = response.status();
                    let body = response.text().await?;
                    if status != reqwest::StatusCode::OK {
                        error!("get_current_block response status: {}, body: {}", status.as_str(), body);
                        error = Some(io::Error::new(ErrorKind::Other, body));
                        continue;
                    }
                    let current_block: Block<N> = serde_json::from_str(&body).unwrap();
                    let latest_block = blocks.take().unwrap();
                    crate::parse::parse_block::<N>(
                        &current_block, 
                        &latest_block.1, 
                        self.address, 
                        self.solution_sender.clone(),
                        self.block_sender.clone(),
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












