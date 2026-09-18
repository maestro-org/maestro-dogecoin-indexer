use crate::{Decode, ShortByteString};
use timbre_xbt_macros::Encode;

#[derive(Clone, Debug, Encode, Decode)]
/// size 26 or 27 (including breaks)
pub struct Key {
    pub script_hash: [u8; 20],
    pub ticker: ShortByteString,
}

// value is big endian u128
