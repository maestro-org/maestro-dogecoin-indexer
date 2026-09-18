use crate::{Decode, Encode, TimbreVec, VarUInt};

#[derive(Clone, Debug, Encode, Decode)]
/// max size 35
pub struct Key {
    // block height
    pub height: VarUInt,
    // index of transaction in block
    pub tx_index: VarUInt,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
/// max size 32 + 1 + 17 + (num of inscriptions * (32 + 17 + 1 + 20 + 17 + 17 + 20 + 17 + 17))
/// expected size 32 + 1 + 4 + (num of inscriptions * (32 + 2 + 1 + 20 + 3 + 3 + 20 + 3 + 3))
pub struct Value {
    pub tx_hash: [u8; 32],
    pub inscriptions_activity: TimbreVec<(
        // (reveal tx hash, index of inscription in reveal tx)
        ([u8; 32], VarUInt),
        (
            // (from address, tx input index, inscribed sat offset)
            // NOTE: this is defined as optional to account for new inscriptions
            Option<([u8; 20], VarUInt, VarUInt)>,
            // (to address, tx output index, inscribed sat offset)
            ([u8; 20], VarUInt, VarUInt),
        ),
    )>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Cursor {
    // tx index in block
    pub tx_index: VarUInt,
    // activity within the tx
    pub activity_index: VarUInt,
}
