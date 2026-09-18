use std::ops::Range;

use crate::{prefix_key_range, rollback, CollectionIngestor, Decode, MinerIngestor, Reducer};

use super::{enc, Encode};

#[derive(Debug, Clone)]
pub struct Namespace {
    pub dataplane_id: u8,
    pub instance_id: u8,
}

impl Namespace {
    pub fn new(dataplane_id: u8, instance_id: u8) -> Self {
        Self {
            dataplane_id,
            instance_id,
        }
    }

    pub fn dataplane_key_range(&self) -> Range<Vec<u8>> {
        prefix_key_range(&[self.dataplane_id])
    }

    pub fn instance_key_range(&self) -> Range<Vec<u8>> {
        prefix_key_range(&self.encode())
    }

    pub fn batch_complete_key_range(&self) -> Range<Vec<u8>> {
        let prefix = enc().append(self).batch_complete_tag().break_().build();

        prefix_key_range(&prefix)
    }
}

impl Encode for Namespace {
    fn encode(&self) -> Vec<u8> {
        enc()
            .append(&self.dataplane_id)
            .append(&self.instance_id)
            .build()
    }
}

#[derive(Debug, Clone)]
pub struct Prefix {
    namespace: Namespace,
}

impl Prefix {
    pub fn new(dataplane_id: u8, instance_id: u8) -> Self {
        Self {
            namespace: Namespace::new(dataplane_id, instance_id),
        }
    }

    pub fn namespace(&self) -> &Namespace {
        &self.namespace
    }

    pub fn cursor(&self) -> Vec<u8> {
        enc().append(&self.namespace).cursor_tag().build()
    }

    pub fn info(&self) -> Vec<u8> {
        enc().info_tag().break_().append(&self.namespace).build()
    }

    pub fn data<T: Encode + Decode>(&self, kind: &Reducer, data: &T) -> Vec<u8> {
        enc()
            .append(&self.namespace)
            .data_tag()
            .append_with_break(kind)
            .append(data)
            .build()
    }

    pub fn collection_metadata<T: Encode>(&self, kind: &CollectionIngestor, data: &T) -> Vec<u8> {
        enc()
            .append(&self.namespace)
            .collection_metadata_tag()
            .append_with_break(kind)
            .append(data)
            .build()
    }

    pub fn miner_metadata<T: Encode>(&self, kind: &MinerIngestor, data: &T) -> Vec<u8> {
        enc()
            .append(&self.namespace)
            .miner_metadata_tag()
            .append_with_break(kind)
            .append(data)
            .build()
    }

    pub fn rollback(&self, data: rollback::MetadataKey) -> Vec<u8> {
        enc()
            .append(&self.namespace)
            .rollback_tag()
            .break_()
            .append(&data)
            .build()
    }
}
