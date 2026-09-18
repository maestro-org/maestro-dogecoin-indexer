use crate::Decode;
use timbre_xbt_macros::Encode;

#[derive(Clone, Debug, Encode, Decode)]
/// size 33
pub struct Key {
    pub script_hash: [u8; 20],
    pub rune_id: (u64, u32),
}

// value is big endian u128
