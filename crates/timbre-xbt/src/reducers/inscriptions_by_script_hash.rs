use crate::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode)]
/// size 57 (including breaks)
pub struct Key {
    // script hash
    pub script_hash: [u8; 20],
    // inscription ID
    pub inscription_id: ([u8; 32], u32),
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
/// size 51 (including breaks)
pub struct Value {
    // block height
    pub height: u64,
    // tx hash of the UTxO containing the inscribed sat
    pub utxo_hash: [u8; 32],
    // tx output index of the UTxO containing the inscribed sat
    pub utxo_index: u32,
    // offset of the inscribed sat within the UTxO
    pub offset: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Cursor {
    // inscription ID
    pub inscription_id: ([u8; 32], u32),
}
