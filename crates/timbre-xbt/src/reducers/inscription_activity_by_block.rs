use crate::{Decode, Encode, VarUInt};

#[derive(Clone, Debug, Encode, Decode)]
/// max size 17
pub struct Key {
    // block height
    pub height: VarUInt,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
/// max size 4 + (number of txs containing inscriptions * (53 + (number of inscriptions in tx * 145)))
pub struct Value {
    pub inscriptions_activity: Vec<(
        // tx index in block
        VarUInt,
        // tx hash
        [u8; 32],
        Vec<(
            // (reveal tx hash, index of inscription in reveal tx)
            ([u8; 32], u32),
            (
                // (from address, tx input index, inscribed sat offset)
                // NOTE: this is defined as optional to account for new inscriptions
                Option<([u8; 20], VarUInt, VarUInt)>,
                // (to address, tx output index, inscribed sat offset)
                ([u8; 20], VarUInt, VarUInt),
            ),
        )>,
    )>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Cursor {
    // tx index in block
    pub index: VarUInt,
}
