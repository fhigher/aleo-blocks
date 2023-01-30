use log::{info, trace};
use tokio::sync::mpsc;

use snarkvm_synthesizer::Block;
use snarkvm_console_network::Network;
use snarkvm_console_types_address::Address;
use snarkos_node_consensus::coinbase_reward;

use crate::message::{Message, Solution, BlockReward, SyncHeight};

pub async fn parse_block<N: Network>(
    current_block: &Block<N>, 
    latest_block: &Block<N>, 
    address: &Vec<String>, 
    sender: mpsc::Sender<Message<N>>,
    store_block: bool,
) -> anyhow::Result<()>{
    let latest_height = latest_block.height();

    let coinbase_solution = current_block.coinbase();
    let next_timestamp = current_block.timestamp();
    let next_height = latest_height.saturating_add(1);
    assert_eq!(next_height, current_block.height(), "current calc block height not equal latest block add one");
    let next_round = latest_block.round().saturating_add(1);
    assert_eq!(next_round, current_block.round(), "current calc block round not equal latest block add one");

    let mut total_reward = 0_u64;
    let mut solutions_num = 0_usize;
    let mut flag = false;
    if let Some(coinbase) = coinbase_solution {
        let partial_solutions = coinbase.partial_solutions();
        solutions_num = partial_solutions.len();
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
            // solutions的address是配置允许的address数组元素，则标记该块的相关信息可入库，并通过异步channel发送，记录该solution
            if address.contains(&partial_solution.address().to_string()) {
                flag = true;
                let data = Solution { 
                    block_height: next_height, 
                    partial_solution: *partial_solution, 
                    solution_reward: prover_reward,
                    timestamp: next_timestamp,
                };
                sender.send(Message::Solution(data)).await?;
            }
        }

        // block reward
        for prover in prover_rewards {
            trace!("prover {} coinbase reward is {}", prover.0, prover.1);
            // total_reward += prover.1;
            total_reward = total_reward.saturating_add(prover.1);
        }

        info!("block {} coinbase reward is {}, total {} solutions", next_height, total_reward, partial_solutions.len());
    } else {
        info!("block {} had no solutions, maybe no reward, empty block", next_height)
    }

    if store_block && flag {
        let block = current_block.clone();
        sender.send(Message::BlockReward(BlockReward { block, block_reward: total_reward, solutions_num })).await?;
    }

    sender.send(Message::SyncHeight(SyncHeight {height: next_height, _p: std::marker::PhantomData})).await?;
    Ok(())
}