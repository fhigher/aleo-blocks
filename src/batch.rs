use anyhow::{anyhow, bail, Result};
use futures::StreamExt;
use parking_lot::RwLock;
use reqwest::Client;
use std::{sync::Arc, vec};
use log::{info, trace};
use std::time::Instant;
use crate::utils::{handle_dispatch_error, log_progress};
use crate::message::Message;
use tokio::sync::mpsc;

use snarkvm_console_network::Network;
use snarkvm_synthesizer::Block;

#[derive(Debug)]
pub struct Batch<'a, N: Network> {
    client: &'a Client,
    base_url: &'a str,
    start_height: u32,
    end_height: Option<u32>,
    address: &'a Vec<String>,
    blocks: RwLock<Option<(u32, Block<N>)>>,
    batch_request: u32,
    batch_concurrent: usize,
    sender: mpsc::Sender<Message<N>>,
    store_block: bool,
}

impl<'a, N:Network> Batch<'a, N> {
    pub fn new(
        client: &'a Client, 
        base_url: &'a str, 
        start_height: u32, 
        end_height: Option<u32>, 
        address: &'a Vec<String>, 
        batch_request: u32,
        batch_concurrent: usize,
        sender: mpsc::Sender<Message<N>>,
        store_block: bool,
    ) -> Self {
        Self { 
            client, 
            base_url, 
            start_height, 
            end_height, 
            address, 
            blocks: RwLock::new(None), 
            batch_request, 
            batch_concurrent, 
            sender,
            store_block,
        }
    }
    /// Loads blocks from a CDN and process them with the given function.
    ///
    /// On success, this function returns the completed block height.
    /// On failure, this function returns the last successful block height (if any), along with the error.
    pub async fn get_blocks(&self) -> Result<u32, (u32, anyhow::Error)> {

        let start_height = self.start_height;
        // Fetch the CDN height.
        let cdn_height = match self.cdn_height().await {
            Ok(cdn_height) => cdn_height,
            Err(error) => return Err((start_height, error)),
        };
        // If the CDN height is less than the start height, return.
        if cdn_height < start_height {
            return Err((
                start_height,
                anyhow!("The given start height ({start_height}) must be less than the CDN height ({cdn_height})"),
            ));
        }

        // If the end height is not specified, set it to the CDN height.
        let end_height = self.end_height.unwrap_or(cdn_height);
        // If the end height is greater than the CDN height, set the end height to the CDN height.
        let end_height = if end_height > cdn_height { cdn_height } else { end_height };
        // If the end height is less than the start height, return.
        if end_height < start_height {
            return Err((
                start_height,
                anyhow!("The given end height ({end_height}) must be less than the start height ({start_height})"),
            ));
        }

        // Compute the CDN start height rounded down to the nearest multiple.
        // let cdn_start = start_height - (start_height % BLOCKS_PER_FILE);
        let cdn_start = start_height;
        // Set the CDN end height to the given end height.
        let cdn_end = end_height;
        // Construct the CDN range.
        let cdn_range = cdn_start..cdn_end;
        // If the CDN range is empty, return.
        if cdn_range.is_empty() {
            return Ok(cdn_end);
        }
        // A tracker for the completed block height.
        let completed_height: Arc<RwLock<u32>> = Arc::new(RwLock::new(start_height));
        // A tracker to indicate if the sync failed.
        let failed: Arc<RwLock<Option<anyhow::Error>>> = Default::default();

        // Start a timer.
        let timer = Instant::now();

        futures::stream::iter(cdn_range.clone().step_by(self.batch_request as usize))
            .map(|start| {
                // Prepare the end height.
                let end = start + self.batch_request;

                // If the sync *has not* failed, log the progress.
                let ctx = format!("blocks {start} to {end}");
                if failed.read().is_none() {
                    info!("Requesting {ctx} (of {cdn_end})");
                }

                // Download the blocks with an exponential backoff retry policy.
                let base_url_clone = self.base_url.to_string();
                let failed_clone = failed.clone();
                handle_dispatch_error(move || {
                    let ctx = ctx.clone();
                    let base_url = base_url_clone.clone();
                    let failed = failed_clone.clone();
                    async move {
                        // If the sync failed, return with an empty vector.
                        if failed.read().is_some() {
                            return std::future::ready(Ok(vec![])).await
                        }
                        // 取指定高度block
                        // let blocks_url = format!("{base_url}/block/{start}");
                        // let blocks: Vec<Block<N>> = cdn_get_one(client, &blocks_url, &ctx).await?;
                        // 取范围内 [start, end)
                        let blocks_url = format!("{base_url}/blocks?start={start}&end={end}");
                        // Fetch the blocks.
                        let blocks: Vec<Block<N>> = self.cdn_get_range(&blocks_url, &ctx).await?;
                        // Return the blocks.
                        std::future::ready(Ok(blocks)).await
                    }
                })
            })
            .buffered(self.batch_concurrent) // The number of concurrent requests.
            .for_each(|result| async {
                // If the sync previously failed, return early.
                if failed.read().is_some() {
                    return;
                }
                // Unwrap the blocks.
                let mut blocks = match result {
                    Ok(blocks) => blocks,
                    Err(error) => {
                        failed.write().replace(error);
                        return;
                    }
                };

                // Only retain blocks that are at or above the start height and below the end height.
                blocks.retain(|block| block.height() >= start_height && block.height() < end_height);

                #[cfg(debug_assertions)]
                // Ensure the blocks are in order by height.
                for (i, block) in blocks.iter().enumerate() {
                    if i > 0 {
                        assert_eq!(block.height(), blocks[i - 1].height() + 1);
                    }
                }
              
                // Fetch the last height in the blocks.
                let curr_height = blocks.last().map(|block| block.height()).unwrap_or(start_height);

                // Process each of the blocks.
                for block in blocks {
                    // Retrieve the block height.
                    let block_height = block.height();
                    // If the sync failed, set the failed flag, and return.
                    if let Err(error) = self.process_block(&block).await {
                        let error = anyhow!("Failed to process block {block_height}: {error}");
                        failed.write().replace(error);
                        return;
                    }

                    // On success, update the completed height.
                    *completed_height.write() = block_height;
                }

                // Log the progress.
                log_progress(timer, curr_height, &cdn_range, "block", self.batch_request);
            })
            .await;

        // Retrieve the successfully completed height (does not include failed blocks).
        let completed = *completed_height.read();
        // Return the result.
        match Arc::try_unwrap(failed).unwrap().into_inner() {
            // If the sync failed, return the completed height along with the error.
            Some(error) => Err((completed, error)),
            // Otherwise, return the completed height.
            None => Ok(completed),
        }
    }

    async fn process_block(&self, current_block: &Block<N>) -> anyhow::Result<()> {
        trace!("current block {}", current_block.height());
        // 刚启动，起始高度已在库里面，并且已计算过奖励，只用于下一个块的计算
        if current_block.height() == self.start_height {
            self.blocks.write().replace((self.start_height, current_block.clone()));
            return Ok(())
        }
    
        // 使用缓存的上一个块，计算当前块
        {
            let latest_block = self.blocks.read().as_ref().unwrap().clone();
            trace!("latest block {}", latest_block.0);
            crate::parse::parse_block::<N>(
                &current_block, 
                &latest_block.1, 
                &self.address, 
                self.sender.clone(),
                self.store_block,
            ).await?;
        }
        
        // 当前块计算完成后，缓存，成为下一个块的计算依赖
        self.blocks.write().replace((current_block.height(), current_block.clone()));
        Ok(())
    }

    /// Retrieves the CDN height with the given base URL.
    ///
    /// Note: This function decrements the tip by a few blocks, to ensure the
    /// tip is not on a block that is not yet available on the CDN.
    async fn cdn_height(&self) -> Result<u32> {
        // Create a request client.
        let client = match reqwest::Client::builder().build() {
            Ok(client) => client,
            Err(error) => bail!("Failed to create a CDN request client: {error}"),
        };
        // Prepare the URL.
        let height_url = format!("{}/latest/height", self.base_url);
        // Send the request.
        let response = match client.get(height_url).send().await {
            Ok(response) => response,
            Err(error) => bail!("Failed to fetch the CDN height: {error}"),
        };
        // Parse the response.
        let text = match response.text().await {
            Ok(text) => text,
            Err(error) => bail!("Failed to parse the CDN height response: {error}"),
        };
        // Parse the tip.
        let tip = match text.parse::<u32>() {
            Ok(tip) => tip,
            Err(error) => bail!("Failed to parse the CDN tip: {error}"),
        };
        // Decrement the tip by a few blocks to ensure the CDN is caught up.
        let tip = tip.saturating_sub(10);
        // Round the tip down to the nearest multiple.
        Ok(tip - (tip % self.batch_request))
    }

    /// Retrieves the objects from the CDN with the given URL.
    #[allow(unused)]
    async fn _cdn_get_one(&self, url: &str, ctx: &str) -> Result<Vec<Block<N>>> {
        // Fetch the bytes from the given URL.
        let response = match self.client.get(url).send().await {
            Ok(response) => response,
            Err(error) => bail!("Failed to fetch {ctx}: {error}"),
        };

        // TODO response 处理
        let res = match response.text().await {
            Ok(body) => serde_json::from_str(&body),
            Err(error) => bail!("Failed to parse {ctx}: {error}"),
        };

        match res {
            Ok(body) => {
                Ok(vec![body])
            },
            Err(error) => bail!("Failed to deserialize {ctx}: {error}"),
        }
    }

    async fn cdn_get_range(&self, url: &str, ctx: &str) -> Result<Vec<Block<N>>> {
        let response = match self.client.get(url).send().await {
            Ok(response) => response,
            Err(error) => bail!("Failed to fetch {ctx}: {error}"),
        };

        // TODO response 处理, 使用tokio::spawn_blocking处理json解码
        let res = match response.text().await {
            Ok(body) => serde_json::from_str(&body),
            Err(error) => bail!("Failed to parse {ctx}: {error}"),
        };

        match res {
            Ok(body) => {
                Ok(body)
            },
            Err(error) => bail!("Failed to deserialize {ctx}: {error}"),
        }
    }
    
}




