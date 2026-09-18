use crate::Decode;
use timbre_xbt_macros::Encode;

#[derive(Clone, Debug, Encode, Decode)]
/// size 12
pub struct Key {
    pub rune_id: (u64, u32),
}

// value is big endian u128
