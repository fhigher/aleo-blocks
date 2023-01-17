use log::{info, trace, error};
use tokio::{sync::mpsc, select};

use snarkvm_synthesizer::{Block, PartialSolution};
use snarkvm_console_network::Network;
use snarkvm_console_types_address::Address;
use snarkos_node_consensus::coinbase_reward;

use crate::storage::{Store, Storage};

pub async fn parse_block<N: Network>(
    current_block: &Block<N>, 
    latest_block: &Block<N>, 
    address: &Vec<String>, 
    solution_sender: mpsc::Sender<Solution<N>>,
    block_sender: mpsc::Sender<BlockReward<N>>,
) -> anyhow::Result<()>{
    let latest_height = latest_block.height();

    let coinbase_solution = current_block.coinbase();
    let next_timestamp = current_block.timestamp();
    let next_height = latest_height.saturating_add(1);
    assert_eq!(next_height, current_block.height(), "current calc block height not equal latest block add one");
    let next_round = latest_block.round().saturating_add(1);
    assert_eq!(next_round, current_block.round(), "current calc block round not equal latest block add one");

    
    if let Some(coinbase) = coinbase_solution {
        let partial_solutions = coinbase.partial_solutions();
        let solutions_num = partial_solutions.len();
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

        let address = address.clone();
        let mut flag = false;
        let mut prover_rewards: Vec<(Address<N>, u64)> = Vec::new();
        // 每个solution的奖励
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

            // 入库存储 
            // 1. solutions的address是配置允许的address数组元素，则标记该块的相关信息可入库，并通过异步channel发送，记录该solution
            // 2. 没有配置指定address数组
            if address.is_empty() || address.contains(&partial_solution.address().to_string()) {
                flag = true;
                solution_sender.send(Solution { block_height: next_height, partial_solution: *partial_solution, solution_reward: prover_reward }).await?;
            }
        }

        // block reward
        let mut total_reward = 0u64;
        for prover in prover_rewards {
            trace!("prover {} coinbase reward is {}", prover.0, prover.1);
            total_reward += prover.1;
        }

        // 区块信息入库
        if flag {
            let block = current_block.clone();
            block_sender.send(BlockReward { block, block_reward: total_reward, solutions_num }).await?;
        }

        info!("block {} coinbase reward is {}, total {} solutions", next_height, total_reward, partial_solutions.len());
    } else {
        info!("block {} had no solutions, maybe no reward, empty block", next_height)
    }

    Ok(())
}

#[derive(Debug)]
pub struct Solution<N: Network> {
    pub block_height: u32,
    pub partial_solution: PartialSolution<N>,
    pub solution_reward: u64,
}

#[derive(Debug)]
pub struct BlockReward<N: Network> {
    pub block: Block<N>,
    pub solutions_num: usize,
    pub block_reward: u64,
}

pub async fn handle<N: Network, S: Storage<N>>(
    store: Store<N, S>, 
    mut solution_receiver: mpsc::Receiver<Solution<N>>,
    mut block_receiver: mpsc::Receiver<BlockReward<N>>,
) {
    loop {
        select! {
            solution = solution_receiver.recv() => {
                if solution.is_none() {
                    error!("receive None from solution channel");
                    continue;
                }
                let s = solution.unwrap();
                if let Err(e) = store.record_solutions(&s) {
                    error!("record block {} solution failed {:?}", s.block_height, e);
                    return 
                }
            }

            block = block_receiver.recv() => {
                if block.is_none() {
                    error!("receive None from block channel");
                    continue;
                }

                let b = block.unwrap();
                if let Err(e) = store.record_block(&b) {
                    error!("record block {} failed {:?}", b.block.height(), e);
                    return 
                }
            }
        }
    }
}