use log::info;

use snarkvm_synthesizer::Block;
use snarkvm_console_network::Network;
use snarkvm_console_types_address::Address;
use snarkos_node_consensus::coinbase_reward;

pub fn parse_block<N: Network>(current_block: &Block<N>, latest_block: &Block<N>) -> Result<(), reqwest::Error>{
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

        info!("block {} coinbase reward is {}, total {} solutions", next_height, total_reward, partial_solutions.len());
    } else {
        info!("block {} had no solutions, maybe no reward, empty block", next_height)
    }
    


    Ok(())
}