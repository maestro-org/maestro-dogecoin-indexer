use std::fmt::Debug;

use crate::{enc, Decode, Encode, Namespace};

#[derive(Debug, Clone, Encode, Decode, PartialEq)]
pub struct MetadataKey {
    pub height: u64,
    pub hash: [u8; 32],
    pub operation_idx: u64,
    pub modified_key: Vec<u8>,
}

pub fn rollback_metadata_key_range<A: Debug + Encode + Clone, B: Debug + Encode + Clone>(
    namespace: &Namespace,
    lower: Option<A>,
    upper: Option<B>,
) -> (Vec<u8>, Vec<u8>) {
    let prefix = enc().append(&namespace).rollback_tag();

    let (empty_start, empty_end) = (
        prefix.clone().break_().build(),
        prefix.clone().break1().build(),
    );

    let start = lower.map(|v| prefix.clone().break_().append(&v).build());
    let end = upper.map(|v| prefix.break_().append(&v).build());

    (start.unwrap_or(empty_start), end.unwrap_or(empty_end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builder::{BREAK, BREAK_1, PREFIX_ROLLBACK},
        *,
    };

    #[test]
    fn test_rollback_key_encoding() {
        let key = MetadataKey {
            height: 3,
            hash: [25; 32],
            operation_idx: 42,
            modified_key: vec![7, 8, 9],
        };

        let encoded = key.encode();

        assert_eq!(
            encoded,
            [
                vec![0u8, 0, 0, 0, 0, 0, 0, 3],
                vec![BREAK],
                vec![25u8; 32],
                vec![BREAK],
                vec![0u8, 0, 0, 0, 0, 0, 0, 42],
                vec![BREAK],
                vec![0, 0, 0, 3, 7, 8, 9]
            ]
            .concat()
        );

        let full_key_encoded = Prefix::new(1, 2).rollback(key);

        assert_eq!(
            full_key_encoded,
            [
                vec![1, 2, PREFIX_ROLLBACK, BREAK],
                vec![0u8, 0, 0, 0, 0, 0, 0, 3],
                vec![BREAK],
                vec![25u8; 32],
                vec![BREAK],
                vec![0u8, 0, 0, 0, 0, 0, 0, 42],
                vec![BREAK],
                vec![0, 0, 0, 3, 7, 8, 9], // Length encoded as u32
            ]
            .concat()
        );
    }

    #[test]
    fn test_encode_rollback_range_cursor() {
        let (start, end) = rollback_metadata_key_range::<u64, _>(
            &Namespace::new(1, 2),
            Some(1),
            Some(MetadataKey {
                height: 3,
                hash: [25; 32],
                operation_idx: 42,
                modified_key: vec![7, 8, 9],
            }),
        );

        assert!(start < end);
        assert_eq!(
            start,
            vec![1, 2, PREFIX_ROLLBACK, BREAK, 0, 0, 0, 0, 0, 0, 0, 1]
        );
        assert_eq!(
            end,
            [
                vec![1, 2, PREFIX_ROLLBACK, BREAK],
                vec![0u8, 0, 0, 0, 0, 0, 0, 3],
                vec![BREAK],
                vec![25u8; 32],
                vec![BREAK],
                vec![0u8, 0, 0, 0, 0, 0, 0, 42],
                vec![BREAK],
                vec![0u8, 0, 0, 3, 7, 8, 9], // Length encoded as u32
            ]
            .concat()
        );
    }

    #[test]
    fn test_encode_rollback_range_full() {
        let (start, end) =
            rollback_metadata_key_range::<u64, u64>(&Namespace::new(1, 2), None, None);

        assert!(start < end);
        assert_eq!(start, vec![1, 2, PREFIX_ROLLBACK, BREAK]);
        assert_eq!(end, vec![1, 2, PREFIX_ROLLBACK, BREAK_1]);
    }

    #[test]
    fn test_encode_rollback_range_lower() {
        let (start, end) =
            rollback_metadata_key_range::<u64, u64>(&Namespace::new(1, 2), Some(1), None);

        assert!(start < end);
        assert_eq!(
            start,
            vec![1, 2, PREFIX_ROLLBACK, BREAK, 0, 0, 0, 0, 0, 0, 0, 1]
        );
        assert_eq!(end, vec![1, 2, PREFIX_ROLLBACK, BREAK_1]);
    }

    #[test]
    fn test_encode_rollback_range_upper() {
        let (start, end) =
            rollback_metadata_key_range::<u64, u64>(&Namespace::new(1, 2), None, Some(1));

        assert!(start < end);
        assert_eq!(start, vec![1, 2, PREFIX_ROLLBACK, BREAK]);
        assert_eq!(
            end,
            vec![1, 2, PREFIX_ROLLBACK, BREAK, 0, 0, 0, 0, 0, 0, 0, 1]
        );
    }

    #[test]
    fn test_encode_rollback_range_both() {
        let (start, end) =
            rollback_metadata_key_range::<u64, u64>(&Namespace::new(1, 2), Some(1), Some(3));

        assert!(start < end);
        assert_eq!(
            start,
            vec![1, 2, PREFIX_ROLLBACK, BREAK, 0, 0, 0, 0, 0, 0, 0, 1]
        );
        assert_eq!(
            end,
            vec![1, 2, PREFIX_ROLLBACK, BREAK, 0, 0, 0, 0, 0, 0, 0, 3]
        );
    }
}
