use crate::{Decode, Encode};

// max size: 20 + 1 + 8 + 1 + 4 + 1 + 32
#[derive(Clone, Debug, Encode, Decode)]
pub struct Key {
    // Script hash.
    pub script_hash: [u8; 20],

    // Block height.
    pub height: u64,

    // Index of tx in the block.
    pub activity_tx_index: u32,

    // Transaction hash.
    pub tx_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
// max size: 1 + 1 + 8
pub struct Value {
    // Whether the sat balance controlled by the script was increased or decreased in the tx.
    pub increased: bool,

    // Amount by which the sat balance controlled by the script was increased / decreased.
    pub amount: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Cursor {
    // Block height.
    pub height: u64,

    // Index of tx in the block.
    pub activity_tx_index: u32,

    // Transaction hash.
    pub tx_hash: [u8; 32],
}
