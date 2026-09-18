use crate::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
/// size: 20
pub struct Key {
    // Script hash.
    pub script_hash: [u8; 20],
}

#[derive(Encode, Decode, Clone, Debug)]
/// size: 8
pub struct Value {
    pub satoshis: u64,
}
