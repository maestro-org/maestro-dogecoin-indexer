use crate::{Decode, Encode, VarUInt};

#[derive(Clone, Debug, Encode, Decode)]
/// max size 35
pub struct Key {
    /// Block height.
    /// Casting type: u64.
    pub height: VarUInt,

    /// Index of this transaction in the block.
    /// Casting type: u32.
    pub tx_index: VarUInt,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
/// size: 32
pub struct Value {
    /// Transaction hash.
    pub tx_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Cursor {
    /// Index of transaction in block.
    /// Casting type: u32.
    pub tx_index: VarUInt,
}
