use mysql::*;
use mysql::prelude::*;

use snarkvm_console_network::Network;

use crate::storage::Storage;
use crate::message::{Solution, BlockReward};

pub struct MysqlClient {
    pool: Pool,
}

const TABLE_BLOCKS_NAME: &str = "blocks";
// const TABLE_TRANSACTIONS_NAME: &str = "transactions";
const TABLE_SOLUTIONS_NAME: &str = "block_solutions";


impl<'a,N> Storage<N> for MysqlClient where N: Network {
    fn new(url: String) -> Self {
        let pool = Pool::new(url.as_str()).unwrap();
        Self {
            pool
        }
    }

    fn record_block(&self, block: &BlockReward<N>) -> anyhow::Result<bool>{
        let mut conn = self.pool.get_conn()?;
        let fields = "block_height,block_hash,previous_block_hash,network,coinbase_target,proof_target,last_coinbase_target,last_coinbase_timestamp,timestamp,solutions_num,block_reward";
        let sql = format!("INSERT INTO {} ({}) VALUES(?,?,?,?,?,?,?,?,?,?,?)", TABLE_BLOCKS_NAME, fields);
        conn.exec_drop(
            sql, 
            (
                block.block.height(),
                block.block.hash().to_string(),
                block.block.previous_hash().to_string(),
                block.block.network(),
                block.block.coinbase_target(),
                block.block.proof_target(),
                block.block.last_coinbase_target(),
                block.block.last_coinbase_timestamp(),
                block.block.timestamp(),
                block.solutions_num,
                block.block_reward
            )
        )?;
        Ok(true)
    }

    fn record_solutions(&self, solution: &Solution<N>) -> anyhow::Result<bool> {
        let mut conn = self.pool.get_conn()?;
        let sql = format!("INSERT INTO {} (block_height, address, nonce, commitment, solution_reward, timestamp) VALUES(?, ?, ?, ?, ?, ?)", TABLE_SOLUTIONS_NAME);
        conn.exec_drop(
            sql, 
            (
                solution.block_height, 
                solution.partial_solution.address().to_string(), 
                solution.partial_solution.nonce(), 
                solution.partial_solution.commitment().to_string(),
                solution.solution_reward,
                solution.timestamp,
            )
        )?;
        Ok(true)
    }
}