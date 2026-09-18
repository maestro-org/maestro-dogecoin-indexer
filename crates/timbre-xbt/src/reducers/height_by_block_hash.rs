use crate::{Decode, Encode, VarUInt};

#[derive(Clone, Debug, Encode, Decode)]
/// size 32
pub struct Key {
    pub block_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
/// max size 17
pub struct Value {
    pub block_height: VarUInt,
}
