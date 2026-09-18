use crate::storage::kvtable::*;

use super::{
    inscriptions::{inscription_id::InscriptionId, updater::BRC20Message, Counters},
    BlockHash, DuneId, NewInscriptionInfo, TxoBody, TxoRef,
};

// block slot -> (hash, required transaction output bytes)
pub struct ResolverByHeightKV;

pub type ResolverValue = Vec<(TxoRef, TxoBody)>;
pub type RunesResolverValue = Vec<(TxoRef, Vec<(DuneId, u128)>)>; // outputs to runes (not inputs)
pub type InscriptonsResolverValue = Vec<(TxoRef, Vec<(u64, InscriptionId)>)>; // outputs to inscriptions and their offsets (not inputs)
                                                                              // pub type LostAndUnboundInscriptions
pub type InscriptionCounters = Counters;
pub type TxEtchs = Vec<(u32, u128)>;
pub type TxMintIndices = Vec<(u32, u16)>;
pub type InscriptionIndices = Vec<(u32, u32)>;
pub type BRC20Resolver = Vec<(InscriptionId, BRC20Message)>;
pub type NewInscriptions = Vec<NewInscriptionInfo>;

// (_, _, _, indices of txs with successful rune etchs, indices of txs with successful rune mints)
impl
    KVTable<
        DBInt,
        DBSerde<(
            BlockHash,
            ResolverValue,
            RunesResolverValue,
            TxEtchs,       // indices of txs with successful rune etchs (and name )
            TxMintIndices, // indices of txs with successful rune mints
            InscriptonsResolverValue,
            InscriptionIndices, // indices of inscriptions with valid reinscriptions
            InscriptionCounters,
            BRC20Resolver,
            NewInscriptions,
        )>,
    > for ResolverByHeightKV
{
    const CF_NAME: &'static str = "ResolverByHeightKV";
}
