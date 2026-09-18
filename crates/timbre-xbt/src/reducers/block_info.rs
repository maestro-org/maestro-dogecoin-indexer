use crate::{Decode, Encode, TimbreVec, VarUInt};

#[derive(Clone, Debug, Encode, Decode)]
/// max size: 17
pub struct Key {
    /// Block height.
    /// Casting type: u64.
    pub height: VarUInt,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
/// max size: 32 + 1 + 17 + 1 + 17 + 1 + 17 + 1 + 17 + 1 + 17 + 1 + 17 + 1 + +1 + 1 + 1 + 1 + 1 + 1 + 17 + length of coinbase script_sig in coinbase tx
pub struct Value {
    /// Block hash.
    pub block_hash: Option<[u8; 32]>,

    /// Block size.
    /// Casting type: u64.
    pub block_size: VarUInt,

    /// Number of weight units (WU) of the block.
    /// Casting type: u64.
    pub block_weight_units: VarUInt,

    /// The timestamp of the block, as claimed by the miner.
    /// Casting type: u32.
    pub timestamp: Option<VarUInt>,

    /// Total fees paid by all transactions in the block.
    /// Casting type: u128.
    pub total_fees: VarUInt,

    /// Total number of satoshis that went through this block, minus fees.
    /// Casting type: u128.
    pub total_volume: VarUInt,

    /// Total number of transactions.
    /// Casting type: u64,
    pub total_txs: VarUInt,

    /// Whether any of the inputs or outputs of any of the transactions in the block contains inscriptions.
    pub involves_inscriptions: bool,

    /// Whether any of the inputs or outputs of any of the transactions in the block contains runes.
    pub involves_runes: bool,

    /// Whether any of the inputs or outputs of any of the transactions in the block contains BRC-20 messages.
    pub involves_brc20: bool,

    /// Miner tag.
    pub coinbase_script_sig: TimbreVec<u8>,
}
