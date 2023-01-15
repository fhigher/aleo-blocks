use anyhow::{anyhow, bail, Result};
use core::ops::Range;
use futures::{Future, StreamExt};
use parking_lot::RwLock;
use reqwest::Client;
use std::{
    sync::Arc,
    time::{Duration, Instant}, vec,
};
use log::{debug, info, trace};
use colored::Colorize;

use snarkvm_console_network::Testnet3;
use snarkvm_synthesizer::Block;

/// The number of blocks per file.
const BLOCKS_PER_FILE: u32 = 10;
const CONCURRENT_REQUEST: usize = 2;

/// Loads blocks from a CDN and process them with the given function.
///
/// On success, this function returns the completed block height.
/// On failure, this function returns the last successful block height (if any), along with the error.
pub async fn get_blocks(
    client: &Client,
    base_url: &str,
    start_height: u32,
    end_height: Option<u32>,
    process: impl FnMut(Block<Testnet3>, u32) -> Result<()> + Clone + Send + Sync + 'static,
) -> Result<u32, (u32, anyhow::Error)> {
    // Fetch the CDN height.
    let cdn_height = match cdn_height::<BLOCKS_PER_FILE>().await {
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
    let end_height = end_height.unwrap_or(cdn_height);
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

    futures::stream::iter(cdn_range.clone().step_by(BLOCKS_PER_FILE as usize))
        .map(|start| {
            // Prepare the end height.
            let end = start + BLOCKS_PER_FILE;

            // If the sync *has not* failed, log the progress.
            let ctx = format!("blocks {start} to {end}");
            if failed.read().is_none() {
                debug!("Requesting {ctx} (of {cdn_end})");
            }

            // Download the blocks with an exponential backoff retry policy.
            let client_clone = client.clone();
            let base_url_clone = base_url.to_string();
            let failed_clone = failed.clone();
            handle_dispatch_error(move || {
                let ctx = ctx.clone();
                let client = client_clone.clone();
                let base_url = base_url_clone.clone();
                let failed = failed_clone.clone();
                async move {
                    // If the sync failed, return with an empty vector.
                    if failed.read().is_some() {
                        return std::future::ready(Ok(vec![])).await
                    }
                    // 取指定高度block
                    // let blocks_url = format!("{base_url}/block/{start}");
                    // let blocks: Vec<Block<Testnet3>> = cdn_get_one(client, &blocks_url, &ctx).await?;
                    // 取范围内 [start, end)
                    let blocks_url = format!("{base_url}/blocks?start={start}&end={end}");
                    // Fetch the blocks.
                    let blocks: Vec<Block<Testnet3>> = cdn_get_range(client, &blocks_url, &ctx).await?;
                    // Return the blocks.
                    std::future::ready(Ok(blocks)).await
                }
            })
        })
        .buffered(CONCURRENT_REQUEST) // The number of concurrent requests.
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

            // Use blocking tasks, as deserialization and adding blocks are expensive operations.
            let mut process_clone = process.clone();
            let cdn_range_clone = cdn_range.clone();
            let completed_height_clone = completed_height.clone();
            let failed_clone = failed.clone();
            let result = tokio::task::spawn_blocking(move || {
                // Fetch the last height in the blocks.
                let curr_height = blocks.last().map(|block| block.height()).unwrap_or(start_height);

                // Process each of the blocks.
                for block in blocks {
                    // Retrieve the block height.
                    let block_height = block.height();

                    // If the sync failed, set the failed flag, and return.
                    if let Err(error) = process_clone(block, start_height) {
                        let error = anyhow!("Failed to process block {block_height}: {error}");
                        failed_clone.write().replace(error);
                        return;
                    }

                    // On success, update the completed height.
                    *completed_height_clone.write() = block_height;
                }

                // Log the progress.
                log_progress::<BLOCKS_PER_FILE>(timer, curr_height, &cdn_range_clone, "block");
            }).await;

            // If the sync failed, set the failed flag.
            if let Err(error) = result {
                let error = anyhow!("Failed to process blocks: {error}");
                failed.write().replace(error);
            }
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

/// Retrieves the CDN height with the given base URL.
///
/// Note: This function decrements the tip by a few blocks, to ensure the
/// tip is not on a block that is not yet available on the CDN.
async fn cdn_height<const BLOCKS_PER_FILE: u32>() -> Result<u32> {
    const BASE_URL: &str = "https://vm.aleo.org/api";

    // Create a request client.
    let client = match reqwest::Client::builder().build() {
        Ok(client) => client,
        Err(error) => bail!("Failed to create a CDN request client: {error}"),
    };
    // Prepare the URL.
    let height_url = format!("{BASE_URL}/testnet3/latest/height");
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
    Ok(tip - (tip % BLOCKS_PER_FILE))
}

/// Retrieves the objects from the CDN with the given URL.
#[allow(unused)]
async fn _cdn_get_one(client: Client, url: &str, ctx: &str) -> Result<Vec<Block<Testnet3>>> {
    // Fetch the bytes from the given URL.
    let response = match client.get(url).send().await {
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

async fn cdn_get_range(client: Client, url: &str, ctx: &str) -> Result<Vec<Block<Testnet3>>> {
    let response = match client.get(url).send().await {
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



/// Logs the progress of the sync.
fn log_progress<const OBJECTS_PER_FILE: u32>(
    timer: Instant,
    current_index: u32,
    cdn_range: &Range<u32>,
    object_name: &str,
) {
    // Prepare the CDN start and end heights.
    let cdn_start = cdn_range.start;
    let cdn_end = cdn_range.end;
    // Compute the percentage completed.
    let percentage = current_index * 100 / cdn_end;
    // Compute the number of files processed so far.
    let num_files_done = 1 + (current_index - cdn_start) / OBJECTS_PER_FILE;
    // Compute the number of files remaining.
    let num_files_remaining = 1 + (cdn_end - current_index) / OBJECTS_PER_FILE;
    // Compute the milliseconds per file.
    let millis_per_file = timer.elapsed().as_millis() / num_files_done as u128;
    // Compute the heuristic slowdown factor (in millis).
    let slowdown = 100 * num_files_remaining as u128;
    // Compute the time remaining (in millis).
    let time_remaining = num_files_remaining as u128 * millis_per_file + slowdown;
    // Prepare the estimate message (in secs).
    let estimate = format!("(est. {} minutes remaining)", time_remaining / (60 * 1000));
    // Log the progress.
    info!("Synced up to {object_name} {current_index} of {cdn_end} - {percentage}% complete {}", estimate.dimmed());
}

/// Executes the given closure, with a backoff policy, and returns the result.
pub(crate) async fn handle_dispatch_error<'a, T, F>(func: impl Fn() -> F + 'a) -> anyhow::Result<T>
where
    F: Future<Output = Result<T, anyhow::Error>>,
{
    use backoff::{future::retry, ExponentialBackoff};

    fn default_backoff() -> ExponentialBackoff {
        ExponentialBackoff {
            max_interval: Duration::from_secs(15),
            max_elapsed_time: Some(Duration::from_secs(60)),
            ..Default::default()
        }
    }

    fn from_anyhow_err(err: anyhow::Error) -> backoff::Error<anyhow::Error> {
        use backoff::Error;

        if let Ok(err) = err.downcast::<reqwest::Error>() {
            debug!("Server error: {err}; retrying...");
            Error::Transient { err: err.into(), retry_after: None }
        } else {
            Error::Transient { err: anyhow!("Block parse error"), retry_after: None }
        }
    }

    retry(default_backoff(), || async { func().await.map_err(from_anyhow_err) }).await
}

lazy_static::lazy_static! {
    static ref BLOCKS:  RwLock<Option<(u32, Block<Testnet3>)>>  = RwLock::new(None);
}

pub fn process_block(current_block: &Block<Testnet3>, start_height: u32) -> anyhow::Result<()> {
    trace!("current block {}", current_block.height());
    // 刚启动，起始高度已在库里面，并且已计算过奖励，只用于下一个块的计算
    if current_block.height() == start_height {
        BLOCKS.write().replace((start_height, current_block.clone()));
        return Ok(())
    }

    // 使用缓存的上一个块，计算当前块
    {
        let latest_block = BLOCKS.read().as_ref().unwrap().clone();
        trace!("latest block {}", latest_block.0);
        crate::parse::parse_block::<Testnet3>(&current_block, &latest_block.1)?;
    }
    
    // 当前块计算完成后，缓存，成为下一个块的计算依赖
    BLOCKS.write().replace((current_block.height(), current_block.clone()));
    Ok(())
}