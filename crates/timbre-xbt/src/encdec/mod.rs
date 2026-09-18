pub mod builder;
pub mod decode;
pub mod encode;
pub mod namespace;

use timbre_xbt_macros::Encode;

pub use self::{decode::Decode, encode::Encode, namespace::*};

pub fn enc() -> builder::EncodeBuilder {
    builder::EncodeBuilder::new()
}

#[cfg(test)]
mod tests {
    use std::u128;

    use indexmap::IndexMap;

    use super::*;
    use crate::*;

    #[test]
    fn test_derive_encode_struct_roundtrip() {
        let original = reducers::utxos_by_rune_id::Key {
            rune_id: (32, 12),
            height: 3,
            utxo_hash: [0x52; 32],
            utxo_index: 0x25,
        };

        let encoded = original.encode();

        let (decoded, _) = reducers::utxos_by_rune_id::Key::decode(&encoded).unwrap();

        assert_eq!(original, decoded)
    }

    #[test]
    fn test_derive_encode_struct_rb_roundtrip() {
        let original = rollback::MetadataKey {
            height: 1337,
            hash: [10; 32],
            operation_idx: 420,
            modified_key: vec![123; 56],
        };

        let encoded = original.encode();

        let (decoded, _) = rollback::MetadataKey::decode(&encoded).unwrap();

        assert_eq!(original, decoded)
    }

    #[test]
    fn test_index_map_roundtrip() {
        let original = vec![(1u8, 1u8), (2u8, 2u8)]
            .into_iter()
            .collect::<IndexMap<_, _>>();

        let encoded = original.encode();

        let (decoded, _) = IndexMap::decode(&encoded).unwrap();

        assert_eq!(original, decoded)
    }

    #[test]
    fn test_varint() {
        assert_eq!(VarUInt(0).encode(), vec![0]);
        assert_eq!(VarUInt(1).encode(), vec![1, 1]);
        assert_eq!(VarUInt(0xFF).encode(), vec![1, 255]);
        assert_eq!(VarUInt(0xFF + 1).encode(), vec![2, 1, 0]);
        assert_eq!(
            VarUInt(u128::MAX).encode(),
            vec![
                16, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255
            ]
        );

        // ---

        let mut x = 0;

        let original = VarUInt(x);
        let encoded = original.encode();
        let decoded = VarUInt::decode(&encoded).unwrap().0;

        assert_eq!(original, decoded);

        for _ in 0..16 {
            x = (x << 8) + 0xFF;

            let original = VarUInt(x);
            let encoded = original.encode();
            let decoded = VarUInt::decode(&encoded).unwrap().0;

            assert_eq!(original, decoded);
        }

        // ---

        assert!(VarUInt(0).encode() < VarUInt(0xFF).encode());
        assert!(VarUInt(0xFF).encode() < VarUInt(0xFF + 1).encode());
        assert!(VarUInt(0xFF).encode() < VarUInt(0xFFFF).encode());
        assert!(VarUInt(0xFF).encode() < VarUInt(0xFFFFFF).encode());
        assert!(VarUInt(0xFF).encode() < VarUInt(0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF).encode());
    }

    #[test]
    fn test_cursor_value_roundtrip() {
        let original = CursorValue {
            height: 1337,
            hash: [10; 32],
            was_mempool: true,
            timestamp: 123456789,
            mempool_info: None,
        };

        let encoded = original.encode();

        let (decoded, _) = CursorValue::decode(&encoded).unwrap();

        assert_eq!(original, decoded)
    }

    #[test]
    fn test_cursor_value_roundtrip_mempool() {
        let original = CursorValue {
            height: 1337,
            hash: [10; 32],
            was_mempool: true,
            timestamp: 123456789,
            mempool_info: Some(((456, [11; 32]), 789)),
        };

        let encoded = original.encode();

        let (decoded, _) = CursorValue::decode(&encoded).unwrap();

        assert_eq!(original, decoded)
    }

    #[test]
    fn test_cursor_value_back_compat() {
        let original = CursorValue {
            height: 1337,
            hash: [10; 32],
            was_mempool: true,
            timestamp: 123456789,
            mempool_info: None,
        };

        let encoded = original.encode();

        assert_eq!(
            (1337u64, [10u8; 32]),
            <(u64, [u8; 32])>::decode(&encoded).unwrap().0
        );
    }

    #[test]
    fn test_impl_try_from_varuint() {
        macro_rules! test_type {
            ($type:ty) => {
                assert_eq!(
                    <$type>::try_from(VarUInt(<$type>::MIN as u128)),
                    Ok(<$type>::MIN),
                );
                assert_eq!(
                    <$type>::try_from(VarUInt(<$type>::MAX as u128)),
                    Ok(<$type>::MAX),
                );
                assert_eq!(
                    <$type>::try_from(VarUInt(<$type>::MAX as u128 + 1u128)),
                    Err(TimbreError::VarUIntCasting(<$type>::MAX as u128 + 1u128)),
                );
            };
        }

        test_type!(usize);
        test_type!(u8);
        test_type!(u16);
        test_type!(u32);
        test_type!(u64);

        // u128 is tested separately because adding 1 to max val would overflow
        assert_eq!(<u128>::try_from(VarUInt(u128::MIN as u128)), Ok(u128::MIN));
        assert_eq!(<u128>::try_from(VarUInt(u128::MAX as u128)), Ok(u128::MAX));
    }

    #[test]
    fn test_timbre_vec() {
        let a = CursorValue {
            height: 1337,
            hash: [10; 32],
            was_mempool: true,
            timestamp: 123456789,
            mempool_info: None,
        };

        let b = CursorValue {
            height: 1337,
            hash: [10; 32],
            was_mempool: true,
            timestamp: 123456789,
            mempool_info: None,
        };

        let original = TimbreVec::from(vec![a, b]);

        let encoded = original.encode();

        let (decoded, _) = <TimbreVec<CursorValue>>::decode(&encoded).unwrap();

        assert_eq!(original, decoded)
    }

    #[test]
    fn test_asset_kind_dogecoin_roundtrip() {
        let bitcoin_asset = crate::AssetKind::Dogecoin;

        let encoded_bitcoin_asset = bitcoin_asset.encode();

        let (decoded, _) = crate::AssetKind::decode(&encoded_bitcoin_asset).unwrap();

        assert_eq!(bitcoin_asset, decoded)
    }

    #[test]
    fn test_asset_kind_dogecoin_encoding() {
        let bitcoin_asset = crate::AssetKind::Dogecoin;

        let encoded_bitcoin_asset = bitcoin_asset.encode();

        let expected_encoding: Vec<u8> = [
            vec![0x00], // asset kind index 0
        ]
        .concat();

        assert_eq!(encoded_bitcoin_asset, expected_encoding)
    }

    #[test]
    fn test_asset_kind_rune_roundtrip() {
        let rune_asset = crate::AssetKind::Rune((VarUInt::from(800000), VarUInt::from(20)));

        let encoded_rune_asset = rune_asset.encode();

        let (decoded, _) = crate::AssetKind::decode(&encoded_rune_asset).unwrap();

        assert_eq!(rune_asset, decoded)
    }

    #[test]
    fn test_asset_kind_rune_encoding() {
        let rune_asset = crate::AssetKind::Rune((VarUInt::from(800000), VarUInt::from(20)));

        let encoded_rune_asset = rune_asset.encode();

        let expected_encoding: Vec<u8> = [
            vec![0x01],                   // asset kind index 1
            vec![0x03, 0x0c, 0x35, 0x00], // varuint 800000
            vec![0x01, 0x14],             // varuint 20
        ]
        .concat();

        assert_eq!(encoded_rune_asset, expected_encoding)
    }

    #[test]
    fn test_struct_like_enum_variant_roundtrip() {
        #[derive(Debug, PartialEq, Encode, Decode)]
        enum TestEnum {
            StructLikeVariant {
                first_field: bool,
                second_field: u64,
            },
        }

        let enum_val = TestEnum::StructLikeVariant {
            first_field: true,
            second_field: 11u64,
        };

        let encoded_enum_val = enum_val.encode();

        let (decoded, _) = TestEnum::decode(&encoded_enum_val).unwrap();
        assert_eq!(enum_val, decoded)
    }
}
