use crate::{Decode, Encode, ShortByteString};

#[derive(Clone, Debug, Encode, Decode)]
/// size 63 or 64 (including breaks)
pub struct Key {
    // Script hash.
    pub script_hash: [u8; 20],
    // Ticker of the transfer inscription.
    pub ticker: ShortByteString,
    // Inscription ID.
    pub inscription_id: ([u8; 32], u32),
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
/// size 77 (including breaks)
pub struct Value {
    // Amount of BRC20 token locked in the transfer inscription.
    pub token_amount: u128,
    // Amount of sat locked in the UTxO.
    pub sat_amount: u64,
    // Tx hash of the UTxO containing the inscribed sat.
    pub utxo_hash: [u8; 32],
    // Tx output index of the UTxO containing the inscribed sat.
    pub utxo_index: u32,
    // Offset of the inscribed sat within the UTxO.
    pub offset: u32,
    // Block height of the transfer inscription.
    pub block_height: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Cursor {
    // Ticker of the transfer inscription.
    pub ticker: ShortByteString,
    // Inscription ID.
    pub inscription_id: ([u8; 32], u32),
}
