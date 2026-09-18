use crate::Decode;
use indexmap::IndexMap;
use timbre_xbt_macros::Encode;

#[derive(Clone, Debug, Encode, Decode)]
/// size 37 (including breaks)
pub struct Key {
    // utxo tx id
    pub utxo_hash: [u8; 32],
    // utxo tx vout
    pub utxo_index: u32,
}

#[derive(Clone, Debug, Encode, Decode)]
/// size 8 + (number of runes * 28)
pub struct Value {
    pub runes: IndexMap<(u64, u32), u128>,
}
