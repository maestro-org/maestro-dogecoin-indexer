use std::{
    collections::HashMap,
    fmt::{self, Debug},
};

use bitcoin::{consensus::Decodable, OutPoint, ScriptHash, TxOut};
use ordinals::RuneId;
use serde::{Deserialize, Serialize};

use crate::{
    crosscut::Point, inscriptions::inscription_id::InscriptionId, prelude::*,
    reducers::ReducerOutput, sources::compressor::compressor_api::OrdinalRange,
};

#[derive(Default, Debug, Clone)]
pub struct BlockContext {
    input_resolver: HashMap<OutPoint, ContextUtxo>,
    runes_resolver: HashMap<OutPoint, Vec<(RuneId, u128)>>,
    pub rune_mint_idxs: Vec<(u32, u32)>,
    pub rune_etch_idxs: Vec<(u32, u128)>,
    inscriptions_resolver: HashMap<OutPoint, Vec<(u32, InscriptionId)>>,
    pub valid_reinscriptions: Vec<(u32, u32)>,
    brc20_resolver: HashMap<InscriptionId, Vec<BRC20Message>>,
    // map for new inscriptions, reveal tx hash -> inscription information
    new_inscriptions: HashMap<[u8; 32], (InscriptionId, u64, Vec<u8>, Vec<u8>)>,
}

#[derive(Debug, Clone)]
pub struct ContextUtxo {
    pub height: u64,
    pub txo: TxOut,
    pub ords: Vec<OrdinalRange>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BRC20Message {
    Deploy(Vec<u8>),
    Mint(Vec<u8>, u128, ScriptHash),
    Transfer(Vec<u8>, u128, OutPoint, ScriptHash),
    TransferInit(Vec<u8>, u128, ScriptHash),
}

impl BlockContext {
    pub fn new() -> Self {
        Self {
            input_resolver: HashMap::new(),
            runes_resolver: HashMap::new(),
            rune_mint_idxs: Vec::new(),
            rune_etch_idxs: Vec::new(),
            inscriptions_resolver: HashMap::new(),
            valid_reinscriptions: Vec::new(),
            brc20_resolver: HashMap::new(),
            new_inscriptions: HashMap::new(),
        }
    }

    pub fn insert_txo(
        &mut self,
        key: &OutPoint,
        height: u64,
        raw: Vec<u8>,
        ords: Vec<OrdinalRange>,
    ) {
        let txo = TxOut::consensus_decode_from_finite_reader(&mut &raw[..]).unwrap();

        let utxo = ContextUtxo { height, txo, ords };

        self.input_resolver.insert(key.clone(), utxo);
    }

    pub fn insert_runes(&mut self, key: &OutPoint, runes: Vec<(RuneId, u128)>) {
        self.runes_resolver.insert(key.clone(), runes);
    }

    pub fn insert_inscriptions(&mut self, key: &OutPoint, inscriptions: Vec<(u32, InscriptionId)>) {
        self.inscriptions_resolver.insert(key.clone(), inscriptions);
    }

    pub fn insert_brc20(&mut self, key: &InscriptionId, action: BRC20Message) {
        self.brc20_resolver.entry(*key).or_default().push(action);
    }

    pub fn find_utxo(&self, key: &OutPoint) -> Result<ContextUtxo, Error> {
        let utxo = self
            .input_resolver
            .get(key)
            .ok_or_else(|| Error::missing_utxo(key))?;

        Ok(utxo.clone())
    }

    pub fn utxo_runes(&self, key: &OutPoint) -> Option<Vec<(RuneId, u128)>> {
        self.runes_resolver.get(key).cloned()
    }

    pub fn utxo_inscriptions(&self, key: &OutPoint) -> Option<Vec<(u32, InscriptionId)>> {
        self.inscriptions_resolver.get(key).cloned()
    }

    pub fn inscription_brc20s(&self, key: &InscriptionId) -> Option<Vec<BRC20Message>> {
        self.brc20_resolver.get(key).cloned()
    }

    pub fn insert_new_inscription(
        &mut self,
        key: &[u8; 32],
        info: (InscriptionId, u64, Vec<u8>, Vec<u8>),
    ) {
        self.new_inscriptions.insert(key.clone(), info);
    }

    pub fn new_inscriptions(
        &self,
        key: &[u8; 32],
    ) -> Option<(InscriptionId, u64, Vec<u8>, Vec<u8>)> {
        self.new_inscriptions.get(key).cloned()
    }
}

#[derive(Debug, Clone)]
pub enum EnrichedBlockPayload {
    RollForward(Point, Vec<u8>, BlockContext, ChainMutable),
    RollBack(Point, ChainMutable),
}

impl EnrichedBlockPayload {
    pub fn roll_forward(
        point: Point,
        block: Vec<u8>,
        ctx: BlockContext,
        mutable: bool,
    ) -> gasket::messaging::Message<Self> {
        gasket::messaging::Message {
            payload: Self::RollForward(point, block, ctx, mutable),
        }
    }

    pub fn roll_back(point: Point, mutable: bool) -> gasket::messaging::Message<Self> {
        gasket::messaging::Message {
            payload: Self::RollBack(point, mutable),
        }
    }
}

type ChainMutable = bool;
type MempoolBlock = bool;

#[derive(Debug, Clone)]
pub enum StorageActionPayload {
    RollForward(Point, Vec<ReducerOutput>, ChainMutable, MempoolBlock),
    RollBack(Point, ChainMutable),
}

pub type Key = Vec<u8>;
pub type Value = Vec<u8>;
pub type DeltaU64 = u64;
pub type DeltaU128 = u128;

pub type Height = u64;

#[derive(Clone)]
#[non_exhaustive]
pub enum StorageAction {
    /// Set `Key` to `Value`
    Set(Key, Value),

    /// Delete the KV pair with key `Key`
    Delete(Key),

    /// Set `Key` to `Value` only if `Key` does not already point to a value
    Insert(Key, Value),

    /// Increment the value at `Key` by `Delta`
    /// (If value at Key exists, it must be big endian u64)
    IncrementU64(Key, DeltaU64),

    /// Decrement the value at `Key` by `Delta`
    /// (If value at Key exists, it must be big endian u64)
    DecrementU64(Key, DeltaU64),

    /// Increment the value at `Key` by `Delta`
    /// (If value at Key exists, it must be big endian u128)
    IncrementU128(Key, DeltaU128),

    /// Decrement the value at `Key` by `Delta`
    /// (If value at Key exists, it must be big endian u128)
    DecrementU128(Key, DeltaU128),

    /// Decrement the value at `Key` by `Delta` (do not remove the key if the resulting value is 0)
    /// (If value at Key exists, it must be big endian u128)
    DecrementU128NoDelete(Key, DeltaU128),
}

impl fmt::Debug for StorageAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Set(k, v) => write!(
                f,
                "StorageAction::Set([{}] -> [{}])",
                hex::encode(&k),
                hex::encode(&v)
            ),
            Self::Insert(k, v) => write!(
                f,
                "StorageAction::Insert([{}] -> [{}])",
                hex::encode(&k),
                hex::encode(&v)
            ),
            Self::Delete(k) => write!(f, "StorageAction::Del({})", hex::encode(&k)),
            Self::IncrementU64(k, d) => {
                write!(f, "StorageAction Incr([{}] += {})", hex::encode(&k), d)
            }
            Self::DecrementU64(k, d) => {
                write!(f, "StorageAction Decr([{}] -= {})", hex::encode(&k), d)
            }
            Self::IncrementU128(k, d) => {
                write!(f, "StorageAction Incr128([{}] += {})", hex::encode(&k), d)
            }
            Self::DecrementU128(k, d) => {
                write!(f, "StorageAction Decr128([{}] -= {})", hex::encode(&k), d)
            }
            Self::DecrementU128NoDelete(k, d) => {
                write!(
                    f,
                    "StorageAction DecrNoD128([{}] -= {})",
                    hex::encode(&k),
                    d
                )
            }
        }
    }
}

impl StorageAction {
    pub fn key(&self) -> &Vec<u8> {
        match self {
            StorageAction::Set(k, _) => k,
            StorageAction::Delete(k) => k,
            StorageAction::Insert(k, _) => k,
            StorageAction::IncrementU64(k, _) => k,
            StorageAction::DecrementU64(k, _) => k,
            StorageAction::IncrementU128(k, _) => k,
            StorageAction::DecrementU128(k, _) => k,
            StorageAction::DecrementU128NoDelete(k, _) => k,
        }
    }

    pub fn into_key(self) -> Vec<u8> {
        match self {
            StorageAction::Set(k, _) => k,
            StorageAction::Delete(k) => k,
            StorageAction::Insert(k, _) => k,
            StorageAction::IncrementU64(k, _) => k,
            StorageAction::DecrementU64(k, _) => k,
            StorageAction::IncrementU128(k, _) => k,
            StorageAction::DecrementU128(k, _) => k,
            StorageAction::DecrementU128NoDelete(k, _) => k,
        }
    }

    /// Returns true if the previous value of the key is required in order to
    /// perform the storage action.
    pub fn requires_previous_value(&self) -> bool {
        match self {
            StorageAction::IncrementU64(_, _) => true,
            StorageAction::DecrementU64(_, _) => true,
            StorageAction::IncrementU128(_, _) => true,
            StorageAction::DecrementU128(_, _) => true,
            StorageAction::DecrementU128NoDelete(_, _) => true,
            StorageAction::Insert(_, _) => true,
            StorageAction::Set(_, _) => false,
            StorageAction::Delete(_) => false,
        }
    }

    /// Panics if StorageActions can't be merged
    pub fn merge(&mut self, other: StorageAction) {
        assert_eq!(
            self.key(),
            other.key(),
            "trying to merge actions with different keys"
        );
        match self {
            Self::IncrementU64(k, pd) => {
                match other {
                    Self::IncrementU64(nk, nd) => *self = Self::IncrementU64(nk, *pd + nd),
                    Self::DecrementU64(nk, nd) => {
                        if *pd >= nd {
                            *self = Self::IncrementU64(nk, *pd - nd)
                        } else {
                            // pd < nd
                            *self = Self::DecrementU64(nk, nd - *pd)
                        }
                    }
                    a => panic!(
                        "unexpected INCR/DECR type pair in storage action merge {a:?} on {k:?}"
                    ),
                }
            }
            Self::DecrementU64(k, pd) => match other {
                Self::DecrementU64(nk, nd) => *self = Self::DecrementU64(nk, *pd + nd),
                Self::IncrementU64(nk, nd) => {
                    if *pd >= nd {
                        *self = Self::DecrementU64(nk, *pd - nd)
                    } else {
                        *self = Self::IncrementU64(nk, nd - *pd)
                    }
                }
                a => panic!("trying to merge SET/DEL/INS on DECR {a:?} {k:?} {pd:?}"),
            },
            Self::IncrementU128(k, pd) => {
                match other {
                    Self::IncrementU128(nk, nd) => *self = Self::IncrementU128(nk, *pd + nd),
                    Self::DecrementU128(nk, nd) => {
                        if *pd >= nd {
                            *self = Self::IncrementU128(nk, *pd - nd)
                        } else {
                            // pd < nd
                            *self = Self::DecrementU128(nk, nd - *pd)
                        }
                    }
                    Self::DecrementU128NoDelete(nk, nd) => {
                        if *pd >= nd {
                            *self = Self::IncrementU128(nk, *pd - nd)
                        } else {
                            // pd < nd
                            *self = Self::DecrementU128NoDelete(nk, nd - *pd)
                        }
                    }
                    a => panic!(
                        "unexpected INCR/DECR type pair in storage action merge {a:?} on {k:?}"
                    ),
                }
            }
            Self::DecrementU128(k, pd) => match other {
                Self::DecrementU128(nk, nd) => *self = Self::DecrementU128(nk, *pd + nd),
                Self::IncrementU128(nk, nd) => {
                    if *pd >= nd {
                        *self = Self::DecrementU128(nk, *pd - nd)
                    } else {
                        *self = Self::IncrementU128(nk, nd - *pd)
                    }
                }
                a => panic!("trying to merge SET/DEL/INS or NoDel on DECR {a:?} {k:?} {pd:?}"),
            },
            Self::DecrementU128NoDelete(k, pd) => match other {
                Self::DecrementU128NoDelete(nk, nd) => {
                    *self = Self::DecrementU128NoDelete(nk, *pd + nd)
                }
                Self::IncrementU128(nk, nd) => {
                    if *pd >= nd {
                        *self = Self::DecrementU128NoDelete(nk, *pd - nd)
                    } else {
                        *self = Self::IncrementU128(nk, nd - *pd)
                    }
                }
                a => panic!("trying to merge SET/DEL/INS or Del on DECR {a:?} {k:?} {pd:?}"),
            },
            Self::Set(k, _) => match other {
                a @ (Self::IncrementU64(_, _)
                | Self::DecrementU64(_, _)
                | Self::IncrementU128(_, _)
                | Self::DecrementU128(_, _)
                | Self::DecrementU128NoDelete(_, _)) => {
                    panic!("trying to merge INCR/DECR on SET {a:?} {k:?}")
                }
                a @ (Self::Set(_, _) | Self::Delete(_)) => *self = a, // overwrite SET with SET/DEL
                Self::Insert(_, _) => (), // do nothing - don't overwrite with an INS
            },
            Self::Delete(k) => match other {
                a @ (Self::IncrementU64(_, _)
                | Self::DecrementU64(_, _)
                | Self::IncrementU128(_, _)
                | Self::DecrementU128(_, _)
                | Self::DecrementU128NoDelete(_, _)) => {
                    panic!("trying to merge INCR/DECR on DEL {a:?} {k:?}")
                }
                // Delete merged with Insert is a Set, because the value is cleared
                Self::Insert(k, v) => *self = Self::Set(k, v),
                // Overwrite DEL with SET/DEL
                a @ (Self::Set(_, _) | Self::Delete(_)) => *self = a,
            },
            Self::Insert(k, _) => match other {
                a @ (Self::IncrementU64(_, _)
                | Self::DecrementU64(_, _)
                | Self::IncrementU128(_, _)
                | Self::DecrementU128(_, _)
                | Self::DecrementU128NoDelete(_, _)) => {
                    panic!("trying to merge INCR/DECR on INS {a:?} {k:?}")
                }
                // Overwrite INS with SET/DEL
                a @ (Self::Set(_, _) | Self::Delete(_)) => *self = a,
                // Don't overwrite an INS with another INS
                Self::Insert(_, _) => (),
            },
        }
    }

    /// Returns the total number of bytes for the key and value (estimate)
    pub fn size(&self) -> usize {
        match self {
            StorageAction::Set(k, v) => k.len() + v.len(),
            StorageAction::Delete(k) => k.len(),
            StorageAction::Insert(k, v) => k.len() + v.len(),
            StorageAction::IncrementU64(k, _) => k.len() + 8,
            StorageAction::DecrementU64(k, _) => k.len() + 8,
            StorageAction::IncrementU128(k, _) => k.len() + 16,
            StorageAction::DecrementU128(k, _) => k.len() + 16,
            StorageAction::DecrementU128NoDelete(k, _) => k.len() + 16,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::StorageAction;

    #[test]
    fn test_merge() {
        let op1 = StorageAction::IncrementU64(vec![1], 10);
        let op2 = StorageAction::DecrementU64(vec![1], 15);

        let mut map = HashMap::new();

        map.insert(vec![1], op1);
        map.insert(vec![2], StorageAction::Set(vec![2], vec![]));

        if let Some(prev) = map.get_mut(op2.key()) {
            prev.merge(op2)
        }

        // an increment of 10 merged with a decrement of 15 nets to a decrement of 5
        assert!(matches!(
            map.get(&vec![1]),
            Some(StorageAction::DecrementU64(_, 5))
        ));
    }
}
