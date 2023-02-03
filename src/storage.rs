use serde::{Serialize, Deserialize};
use snarkvm_console_network::Network;

use std::marker::PhantomData;
use crate::message::{Solution, BlockReward};

#[derive(Debug, Serialize, Deserialize)]
pub struct Reward {
    pub address: String,
    pub height: u32,
    pub nonce: u64,
    pub reward: u64,
    pub timestamp: i64,
}

pub trait Storage<N: Network> {
    fn new(url: String) -> Self; 
    fn record_block(&self, block: &BlockReward<N>) -> anyhow::Result<bool>;
    fn record_solutions(&self, solutions: &Solution<N>) -> anyhow::Result<bool>;
    fn get_solutions_by_time_range(&self, address: &String, begin: i64, end: i64) -> anyhow::Result<Vec<Reward>>;
}

#[derive(Debug, Clone)]
pub struct Store<N:Network, S: Storage<N> + Sized> {
    inner: S,
    _n: PhantomData<N>,
    _s: PhantomData<S>,
}

impl<N:Network, S: Storage<N>> Store<N, S> {
    pub fn new(url: String) -> Self {
        Self {
            inner: S::new(url),
            _n: PhantomData,
            _s: PhantomData,
        }
    }

    pub fn record_block(&self, block: &BlockReward<N>) -> anyhow::Result<bool> {
        self.inner.record_block(block)
    }

    pub fn record_solutions(&self, solution: &Solution<N>) -> anyhow::Result<bool> {
        self.inner.record_solutions(solution)
    }

    pub fn get_solutions_by_time_range(&self, address: &String, begin: i64, end: i64) -> anyhow::Result<Vec<Reward>> {
        self.inner.get_solutions_by_time_range(address, begin, end)
    }
}