use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::Transaction;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct BlockHash([u8; 32]);

impl From<bitcoincore_rpc_async::bitcoin::BlockHash> for BlockHash {
    fn from(value: bitcoincore_rpc_async::bitcoin::BlockHash) -> Self {
        Self(value.to_vec().try_into().unwrap())
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Block {
    /// The block header
    pub header: BlockHeader,
    /// List of transactions contained in the block
    pub txdata: Vec<Transaction>,
}

impl From<bitcoincore_rpc_async::bitcoin::Block> for Block {
    fn from(value: bitcoincore_rpc_async::bitcoin::Block) -> Self {
        Self {
            header: value.header.into(),
            txdata: value.txdata.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct BlockHeader {
    /// The protocol version. Should always be 1.
    pub version: i32,
    /// Reference to the previous block in the chain
    pub prev_blockhash: [u8; 32],
    /// The root hash of the merkle tree of transactions in the block
    pub merkle_root: [u8; 32],
    /// The timestamp of the block, as claimed by the miner
    pub time: u32,
    /// The target value below which the blockhash must lie, encoded as a
    /// a float (with well-defined rounding, of course)
    pub bits: u32,
    /// The nonce, selected to obtain a low enough blockhash
    pub nonce: u32,
}

impl From<bitcoincore_rpc_async::bitcoin::BlockHeader> for BlockHeader {
    fn from(value: bitcoincore_rpc_async::bitcoin::BlockHeader) -> Self {
        Self {
            version: value.version,
            prev_blockhash: value.prev_blockhash.to_vec().try_into().unwrap(),
            merkle_root: value.merkle_root.to_vec().try_into().unwrap(),
            time: value.time,
            bits: value.bits,
            nonce: value.nonce,
        }
    }
}
