use crate::{Decode, Encode, VarUInt};

#[derive(Clone, Debug, Encode, Decode)]
/// max size: 32 + 1 + 17
pub struct Key {
    /// Hash of the transaction that produced this output.
    pub utxo_tx_hash: [u8; 32],

    /// Output index.
    /// Casting type: u32.
    pub utxo_vout: VarUInt,
}

#[derive(Clone, Debug, Encode, Decode)]
/// (max) size: 35
pub struct Value {
    /// Block height.
    /// Casting type: u64.
    pub height: VarUInt,

    /// Index of this transaction in the block.
    /// Casting type: u64.
    pub tx_index: VarUInt,
}
