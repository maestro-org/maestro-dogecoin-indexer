use crate::{enc, MinerIngestor, Namespace};

/// Akin to `reducer_key_range`, but it uses the miners metadata prefix and always returns the
/// entire range.
pub fn miner_metadata_key_range(namespace: &Namespace) -> (Vec<u8>, Vec<u8>) {
    let e = enc()
        .append(&namespace)
        .miner_metadata_tag()
        .append(&MinerIngestor);

    (e.clone().break_().build(), e.clone().break1().build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_miner_metadata_key_range() {
        let (start, end) = miner_metadata_key_range(&Namespace::new(1, 2));

        assert!(start < end);
    }
}
