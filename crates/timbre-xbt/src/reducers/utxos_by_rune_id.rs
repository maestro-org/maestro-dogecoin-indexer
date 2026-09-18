use crate::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
/// size 59 (including breaks)
pub struct Key {
    // (block, tx) of etching
    pub rune_id: (u64, u32),
    // block height
    pub height: u64,
    // utxo tx id
    pub utxo_hash: [u8; 32],
    // utxo tx vout
    pub utxo_index: u32,
}

#[derive(Encode, Decode, Clone, Debug)]
/// size 46 (including breaks)
pub struct Value {
    pub script_hash: [u8; 20],
    pub rune_quantity: u128,
    pub satoshis: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Cursor {
    pub height: u64,
    pub utxo_hash: [u8; 32],
    pub utxo_index: u32,
}

impl Cursor {
    pub fn new(height: u64, utxo_hash: [u8; 32], utxo_index: u32) -> Self {
        Self {
            height,
            utxo_hash,
            utxo_index,
        }
    }
}
