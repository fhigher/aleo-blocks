use log::debug;
use tokio::sync::mpsc;
use log::{error, trace};
use memmap2::MmapMut;

use snarkvm_synthesizer::{Block, PartialSolution};
use snarkvm_console_network::Network;

use crate::storage::Storage;
use crate::storage::Store;

pub trait MessageTrait<N: Network> {
    fn name(&self) -> String;
}

#[derive(Debug)]
pub enum Message<N: Network> {
    Solution(Solution<N>),
    BlockReward(BlockReward<N>),
    SyncHeight(SyncHeight<N>),
}

impl<N: Network> Message<N>  {
    pub fn name(&self) -> String {
        match self {
            Self::Solution(msg) => msg.name(),
            Self::BlockReward(msg) => msg.name(),
            Self::SyncHeight(msg) => msg.name(),
        }
    }
}

#[derive(Debug)]
pub struct Solution<N: Network> {
    pub block_height: u32,
    pub partial_solution: PartialSolution<N>,
    pub solution_reward: u64,
}

impl<N: Network> MessageTrait<N> for Solution<N> {
    fn name(&self) -> String {
        String::from("solution")
    }
}

#[derive(Debug)]
pub struct BlockReward<N: Network> {
    pub block: Block<N>,
    pub solutions_num: usize,
    pub block_reward: u64,
}

impl<N: Network> MessageTrait<N> for BlockReward<N> {
    fn name(&self) -> String {
        String::from("block_reward")
    }
}

#[derive(Debug)]
pub struct SyncHeight<N: Network> {
    pub height: u32,
    pub _p: std::marker::PhantomData<N>
}

impl<N: Network> MessageTrait<N> for SyncHeight<N> {
    fn name(&self) -> String {
        String::from("sync_height")
    }
}

pub async fn handle<N: Network, S: Storage<N>>(
    store: Store<N, S>, 
    mut receiver: mpsc::Receiver<Message<N>>,
    mut mmap: MmapMut,
) {
    debug!("start to listen to message...");
    loop {
        let message = receiver.recv().await; 
        if message.is_none() {
            error!("receive None from message channel");
            continue;
        }
        let message = message.unwrap();
        trace!("receive {} message", message.name());
        match message {
            Message::Solution(msg) => {
                if let Err(e) = store.record_solutions(&msg) {
                    error!("record block {} solution failed {:?}", msg.block_height, e);
                    return 
                }
            },
            Message::BlockReward(msg) => {
                if let Err(e) = store.record_block(&msg) {
                    error!("record block {} failed {:?}", msg.block.height(), e);
                    return 
                }
            },
            Message::SyncHeight(msg) => {
                let height = u32::to_le_bytes(msg.height);
                mmap.copy_from_slice(&height[..]);
            }
        }
    }
}