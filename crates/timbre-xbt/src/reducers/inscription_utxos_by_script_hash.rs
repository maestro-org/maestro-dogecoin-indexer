use crate::{Decode, Encode, TimbreVec, VarUInt};

#[derive(Clone, Debug, Encode, Decode)]
/// max size 89 (including breaks between fields)
pub struct Key {
    // script hash
    pub script_hash: [u8; 20],
    // block height
    pub height: VarUInt,
    // tx hash of the UTxO containing the inscribed sat
    pub utxo_hash: [u8; 32],
    // tx output index of the UTxO containing the inscribed sat
    pub utxo_index: VarUInt,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
/// max size: 35 + (number of inscriptions * 66) (including breaks between fields)
pub struct Value {
    pub satoshis: VarUInt,
    // list of inscriptions held in the UTxO, each one consisting in:
    //      - offset in the UTxO of the inscribed sat,
    //      - the inscription ID (reveal tx hash, index of new inscription in reveal tx)
    pub inscriptions: TimbreVec<(VarUInt, ([u8; 32], VarUInt))>,
}

// User-facing endpoint paginates according to lexicographical order of inscriptions
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Cursor {
    // reveal tx hash
    pub tx_id: [u8; 32],
    // index of new inscription in reveal tx
    pub index: VarUInt,
}
