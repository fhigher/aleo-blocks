use log::{error, debug};
use std::collections::HashMap;

use backoff::future::retry;
use crate::utils::{backoffset, from_reqwest_err};

use snarkvm_console_network::Testnet3;
use snarkvm_synthesizer::Block;
use snarkvm_console_network::Network;
use snarkvm_console_types_address::Address;
use snarkos_node_consensus::coinbase_reward;

pub async fn parse_block<N: Network>(current_block: &Block<N>, latest_block: &Block<N>) -> Result<(), reqwest::Error>{
    let latest_height = latest_block.height();

    
    let coinbase_solution = current_block.coinbase();
    let next_timestamp = current_block.timestamp();
    let next_height = latest_height.saturating_add(1);
    assert_eq!(next_height, current_block.height(), "current calc block height not equal latest block add one");
    let next_round = latest_block.round().saturating_add(1);
    assert_eq!(next_round, current_block.round(), "current calc block round not equal latest block add one");


    if let Some(coinbase) = coinbase_solution {
        let partial_solutions = coinbase.partial_solutions();
        let cumulative_proof_target: u128 = partial_solutions.iter().fold(0u128, |cumulative, solution| {
            cumulative.checked_add(solution.to_target().unwrap() as u128).unwrap()
        });

        let coinbase_reward = coinbase_reward(
            latest_block.last_coinbase_timestamp(),
            next_timestamp,
            next_height,
            N::STARTING_SUPPLY,
            N::ANCHOR_TIME,
        ).unwrap();

        let mut prover_rewards: Vec<(Address<N>, u64)> = Vec::new();
        for partial_solution in partial_solutions {
            // Prover compensation is defined as:
            //   1/2 * coinbase_reward * (prover_target / cumulative_prover_target)
            //   = (coinbase_reward * prover_target) / (2 * cumulative_prover_target)

            // Compute the numerator.
            let numerator = (coinbase_reward as u128)
                .checked_mul(partial_solution.to_target().unwrap() as u128).unwrap();

            // Compute the denominator.
            let denominator = cumulative_proof_target.checked_mul(2).unwrap();

            // Compute the prover reward.
            let prover_reward = u64::try_from(
                numerator.checked_div(denominator).unwrap(),
            ).unwrap();

            prover_rewards.push((partial_solution.address(), prover_reward));
        }

        let mut total_reward = 0u64;
        for prover in prover_rewards {
            //trace!("prover {} coinbase reward is {}", prover.0, prover.1);
            total_reward += prover.1;
        }

        debug!("block {} coinbase reward is {}, total {} solutions", next_height, total_reward, partial_solutions.len());
    } else {
        debug!("block {} had no solutions, maybe no reward, empty block", next_height)
    }
    


    Ok(())
}

pub async fn get_blocks(api: String, latest_height: u32) {
    let mut latest_height_mut = latest_height;
    let client = reqwest::Client::builder().build().unwrap();
    // 需要定时清理
    let mut blocks: HashMap<u32, Block<Testnet3>> = HashMap::new();
    let mut chain_height;
    let mut result;

    loop { 
        result = get_chain_height(&api, &client).await;
        match result {
            Ok(response) => {
                let body = response.text().await.unwrap();
                chain_height = body.parse::<u32>().unwrap();
                debug!("latest chain height: {}", chain_height);
            },
            Err(e) => {
                error!("get chain height {:?}" , e);
                return
            }
        }
        // 链上最新高度 - latest_height_mut <= 15, 则跳过，避免分叉
        if chain_height - latest_height_mut <= 5 {
            // sleep 出块时间
            debug!("recorded height: {}, waiting...", latest_height_mut);
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
            continue;
        }

        // 进程刚启动执行一次，从数据库拿取的latest_height对应的block
        if latest_height_mut == latest_height {
            debug!("get first block: {}", latest_height);
            result = get_block(&api, &client, latest_height).await;
            match result {
                Ok(response)=> {
                    let body = response.text().await.unwrap();
                    let latest_block = serde_json::from_str(&body).unwrap();
                    blocks.insert(latest_height, latest_block);
                },
                Err(e) => {
                    error!("get first block {}, {:?}", latest_height , e);
                    return
                }
            }
        }
        
        // 获取next_height 对应的block, 并计算该block奖励
        let next_height = latest_height_mut + 1;  
        debug!("get next block: {}", next_height);  
        result = get_block(&api, &client, next_height).await;
        match result {
            Ok(response )=> {
                let body = response.text().await.unwrap();
                let current_block: Block<Testnet3> = serde_json::from_str(&body).unwrap();
                let latest_block = blocks.get(&latest_height_mut).unwrap();
                if let Err(e) = parse_block::<Testnet3>(&current_block, latest_block).await {
                    error!("parse block {}, {:?}", next_height, e);
                    return
                }
                latest_height_mut = next_height;
                blocks.insert(next_height, current_block);
            },
            Err(e) => {
                error!("loop get block {}, {:?}", next_height, e);
                return
            }
        }
    }
     
}

/* async fn get_block_once(api: &String, client: &reqwest::Client, height: u32) 
    -> Result<String, reqwest::Error> {
    let api = format!("{}/block/{}", api, height);
    let res = client.get(api)
        .send()
        .await?;
    Ok(res.text().await?)
} */

async fn get_block(api: &String, client: &reqwest::Client, height: u32) 
    -> Result<reqwest::Response, reqwest::Error> {
    retry(backoffset(), || async {
        let block_api = format!("{}/block/{}", api, height);
        client.get(block_api).send().await.map_err(from_reqwest_err) 
    }).await
}

async fn get_chain_height(api: &String, client: &reqwest::Client) 
    -> Result<reqwest::Response, reqwest::Error> {
    retry(backoffset(), || async {
        let height_api = format!("{}/latest/height", api);
        client.get(height_api).send().await.map_err(from_reqwest_err) 
    }).await
}





