use crate::{Decode, Encode, VarUInt};

#[derive(Clone, Debug, Encode, Decode)]
/// size 32
pub struct Key {
    pub tx_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
/// max size 35 (including breaks)
pub struct Value {
    // block height
    pub height: VarUInt,
    // tx index in block
    pub index: VarUInt,
}
