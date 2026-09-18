use crate::Decode;
use timbre_xbt_macros::Encode;

#[derive(Clone, Debug, Encode, Decode)]
/// size 65 (including breaks)
pub struct Key {
    pub script_hash: [u8; 20],
    pub height: u64,
    pub blk_index: u16,
    pub tx_hash: [u8; 32],
}

#[derive(Clone, Debug, Encode, Decode)]
/// size 3 (including breaks)
pub struct Value {
    pub input: bool,
    pub output: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Cursor {
    pub height: u64,
    pub blk_index: u16,
    pub tx_hash: [u8; 32],
}

impl Cursor {
    pub fn new(height: u64, blk_index: u16, tx_hash: [u8; 32]) -> Self {
        Self {
            height,
            blk_index,
            tx_hash,
        }
    }
}
