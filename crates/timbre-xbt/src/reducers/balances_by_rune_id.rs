use crate::{Decode, VarUInt};
use timbre_xbt_macros::Encode;

#[derive(Clone, Debug, Encode, Decode)]
/// max size 55
pub struct Key {
    pub rune_id: (VarUInt, VarUInt),
    pub script_hash: [u8; 20],
}

// value is u128
