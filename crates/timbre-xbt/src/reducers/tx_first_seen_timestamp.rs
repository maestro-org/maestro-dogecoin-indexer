use crate::{Decode, Encode, VarUInt};

#[derive(Clone, Debug, Encode, Decode)]
// max size: 32
pub struct Key {
    pub tx_hash: [u8; 32],
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
// max size: 17
pub struct Value {
    /// The timestamp of the first time the tx was seen.
    /// Casting type: u64.
    pub timestamp: VarUInt,
}
