use crate::{Decode, Encode, TimbreError, VarUInt};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
/// max size: 20 + 1 + 17 + 1 + 32 + 1 + 17
pub struct Key {
    // Script hash.
    pub script_hash: [u8; 20],

    // Block height.
    // Casting type: u64.
    pub height: VarUInt,

    // Tx hash of the UTxO containing the runes.
    pub utxo_hash: [u8; 32],

    // Tx output index of the UTxO containing the runes.
    // Casting type: u32.
    pub utxo_index: VarUInt,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
/// max size: 17 + 1 + 4 + (number of different rune kinds * 51)
pub struct Value {
    // Satoshis locked in the UTxO.
    // Casting type: u64.
    pub satoshis: VarUInt,

    // Runes contained in the UTxO, in the form of ((etching block, etching tx), amount).
    // Casting type: ((u64, u32), u128).
    pub runes: Vec<((VarUInt, VarUInt), VarUInt)>,
}

// Pagination by height or by rune amount.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub enum Cursor {
    ByHeight {
        // Block height.
        height: VarUInt,

        // Tx hash of the UTxO containing the runes.
        utxo_hash: [u8; 32],

        // Tx output index of the UTxO containing the runes.
        utxo_index: VarUInt,
    },
    ByAmount {
        // Amount of relevant runes.
        amount: VarUInt,

        // Tx hash of the UTxO containing the runes.
        utxo_hash: [u8; 32],

        // Tx output index of the UTxO containing the runes.
        utxo_index: VarUInt,
    },
}
