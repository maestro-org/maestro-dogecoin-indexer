use crate::{Decode, Encode};

// max size: 20 + 1 + 8 + 1 + 4 + 1 + 32
#[derive(Clone, Debug, Encode, Decode)]
pub struct Key {
    // Script hash.
    pub script_hash: [u8; 20],

    // Block height.
    pub height: u64,

    // Index of tx with rune activity involving this script hash in the block.
    pub activity_tx_index: u32,

    // Transaction hash.
    pub tx_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
// max size: (1 + 8 + 4 + (1 + 16)) + 1 + (1 + 8 + 4) + 1 + (4 + number of runes with increased balance for this script hash * (8 + 4 + 16)) + 1 + (4 + number of runes with decreased balance for this script hash * (8 + 4 + 16))
pub struct Value {
    // Etched runes, as rune ID and premined runes amount.
    pub etched: Option<((u64, u32), Option<u128>)>,

    // Minted runes, if any, as rune ID. Minted amount should be taken from etching terms for this
    // specific rune kind.
    pub minted: Option<(u64, u32)>,

    // Increased rune balance after this tx, as rune ID and amount of received runes.
    pub increased_balances: Vec<((u64, u32), u128)>,

    // Decreased rune balances after this tx, as rune ID and amount of sent runes.
    pub decreased_balances: Vec<((u64, u32), u128)>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Cursor {
    // Block height.
    pub height: u64,

    // Index of tx with rune activity in the block.
    pub activity_tx_index: u32,

    // Transaction hash.
    pub tx_hash: [u8; 32],
}
