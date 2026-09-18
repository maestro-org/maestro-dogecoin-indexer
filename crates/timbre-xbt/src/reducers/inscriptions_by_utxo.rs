use crate::Decode;
use timbre_xbt_macros::Encode;

#[derive(Clone, Debug, Encode, Decode)]
/// size 37 (including breaks)
pub struct Key {
    // utxo tx id
    pub utxo_hash: [u8; 32],
    // utxo tx vout
    pub utxo_index: u32,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
/// size 4 + (number of inscriptions * 40)
pub struct Value {
    pub inscriptions: Vec<(u32, ([u8; 32], u32))>,
}

#[cfg(test)]
mod tests {
    use crate::Encode;

    use super::*;

    #[test]
    fn test_inscription_serialization() {
        let inscription1 = (123, ([0; 32], 456));
        let inscription2 = (345, ([1; 32], 678));

        let inscriptions = vec![inscription1, inscription2];

        let value = Value { inscriptions };

        let encoded = value.encode();
        let decoded = Value::decode(&encoded).unwrap().0;

        assert_eq!(value, decoded);
    }
}
