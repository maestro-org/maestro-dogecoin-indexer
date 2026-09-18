use crate::{Decode, ShortByteString};
use timbre_xbt_macros::Encode;

#[derive(Clone, Debug, Encode, Decode)]
/// size 5 or 6
pub struct Key {
    pub ticker: ShortByteString,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
/// size 74 (including breaks)
pub struct Value {
    pub max: u128,
    pub limit: u128,
    pub dec: u8,
    pub self_mint: bool,
    pub deploy_id: ([u8; 32], u32),
}
