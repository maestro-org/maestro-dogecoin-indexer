// use ::ordinals::{Artifact, Runestone};
// use bitcoin::consensus::Decodable;
use bitcoin::hashes::Hash;
use bitcoin::{consensus::Encodable, Block, BlockHash, Txid};
use inscriptions::inscription_id::InscriptionId;
use inscriptions::updater::BRC20Message;
use inscriptions::{
    BRC20Balances, InscriptionCountersKV, PartialTxidToBytesKV, PartialTxidToTxidsKV,
    ScriptAndBRC20Kind, SupplyByBRC20, TermsByBRC20Ticker, INSCRIPTION_COUNTERS_KEY,
};
use real_bitcoin::consensus::{Decodable, Encodable as OtherEncodable};
use runes::dunes::claim;
use runes::dunes::dunestone::Dunestone;
use runes::{dunes, DunesReservedCountersKV, DUNES_RESERVED_COUNTERS_KEY};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod chain;
pub mod inscriptions;
pub mod kvtable;
pub mod mutable;
pub mod options;
pub mod resolver;
pub mod runes;
pub mod txos;

use chain::BlockByHeightKV;
use kvtable::*;
use mutable::{Log, MutableKV};
use tracing::{debug, info, warn};
use txos::TxoKV;

type OutputIndex = u32;
pub type BlockValue = (BlockHash, BlockBody);
type BlockHeight = u64;
type BlockBody = Vec<u8>;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct TxoRef(pub Txid, pub OutputIndex);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TxoBody {
    pub height: BlockHeight,
    pub raw: Vec<u8>,
    pub ord_ranges: OrdinalRanges,
    pub dunes: Vec<(DuneId, u128)>,
    pub inscriptions: Vec<(u64, InscriptionId)>,
    pub unused_brc20_transfers: HashMap<InscriptionId, (Vec<u8>, u128)>, // _, ticker, amount
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DuneId {
    pub block: u64,
    pub tx: u64,
}

impl From<dunes::dune_id::DuneId> for DuneId {
    fn from(value: dunes::dune_id::DuneId) -> Self {
        DuneId {
            block: value.height,
            tx: value.index.into(),
        }
    }
}

impl From<DuneId> for dunes::dune_id::DuneId {
    fn from(value: DuneId) -> Self {
        dunes::dune_id::DuneId {
            height: value.block,
            index: value.tx as u32,
        }
    }
}

impl From<DuneId> for Box<[u8]> {
    fn from(value: DuneId) -> Self {
        let mut buf = Vec::with_capacity(16);

        buf.extend_from_slice(&value.block.to_be_bytes());
        buf.extend_from_slice(&value.tx.to_be_bytes());

        buf.into()
    }
}

impl From<Box<[u8]>> for DuneId {
    fn from(value: Box<[u8]>) -> Self {
        let block: [u8; 8] = value[0..8].try_into().unwrap();
        let block = u64::from_be_bytes(block);

        let tx: [u8; 8] = value[8..16].try_into().unwrap();
        let tx = u64::from_be_bytes(tx);

        Self { block, tx }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("RocksDb error: {0}")]
    Rocks(rocksdb::Error),

    #[error("Missing block on undo: {0:?}")]
    UndoMissingBlock(BlockHash),

    #[error("Missing block on intersect: {0:?}")]
    IntersectMissingBlock(BlockHeight),

    #[error("Failed to decode data: {0}")]
    Decoding(String),

    #[error("Failed to encode data: {0}")]
    BitcoinEncode(std::io::Error),

    #[error("Failed to decode data: {0}")]
    BitcoinDecode(bitcoin::consensus::encode::Error),

    #[error("Malformed genesis txo: {0}")]
    GenesisTxo(String),

    #[error("Ordinals: {0}")]
    Ordinals(ordinals::Error),

    #[error("Missing txo to resolve for block {0}: {1}#{2}")]
    MissingTxo(BlockHash, Txid, OutputIndex),

    #[error("Missing resolver entry for block {0:?}")]
    MissingResolver(BlockHash),

    #[error("Resolver for slot {0} hash mismatch: {1:?} v {2:?}")]
    ResolverHashMismatch(u64, BlockHash, BlockHash),

    #[error("Invalid rollback point, found: ({0:?})")]
    InvalidRollbackPoint(Option<(u64, BlockHash)>),

    #[error("Applying block {applying:?} does not immediately follow last block ({received_prev:?} != {expected_prev:?})")]
    PreviousHashMismatch {
        applying: BlockHash,
        received_prev: BlockHash,
        expected_prev: Option<BlockHash>,
    },

    #[error("Ordinals: {0}")]
    Internal(String),
}

impl Into<String> for Error {
    fn into(self) -> String {
        self.to_string()
    }
}

use std::{collections::HashMap, path::Path, sync::Arc, time};

use rocksdb::{BoundColumnFamily, OptimisticTransactionDB, Options, Transaction, DB};

use crate::ordinals::{self, OrdinalRanges};
use crate::storage::runes::{DuneIdByNameKV, DuneMintsByIdKV, DuneTermsByIdKV};
use crate::storage::txos::ResolveAndProcessResult;
use crate::sync::roll::UtxoResolver;
use crate::sync::BitcoinCompatibleNetwork;

use self::resolver::ResolverByHeightKV;

#[derive(Clone)]
pub struct ChainDB {
    pub db: Arc<OptimisticTransactionDB>,
    notifier: Arc<tokio::sync::Notify>,
    next_wal_seq: u64,
    last_block_hash: Option<BlockHash>,
    pub immutable_after_confs: Option<u64>,
    pub network: BitcoinCompatibleNetwork,
    pub first_rune_height: u64,
    pub first_inscription_height: u64,
    pub jubilee_height: u64,
}

impl ChainDB {
    pub fn open(
        path: &std::path::PathBuf,
        immutable_after_confs: Option<u64>,
        network: BitcoinCompatibleNetwork,
        first_rune_height: u64,
        first_inscription_height: u64,
        jubilee_height: u64,
        load: bool,
    ) -> Result<Self, Error> {
        let opts = crate::storage::options::db_options(load);

        let now = time::Instant::now();

        let cfs = [
            rocksdb::ColumnFamilyDescriptor::new(BlockByHeightKV::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(MutableKV::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(ResolverByHeightKV::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(TxoKV::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(DuneIdByNameKV::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(DuneMintsByIdKV::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(DuneTermsByIdKV::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(InscriptionCountersKV::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(PartialTxidToTxidsKV::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(PartialTxidToBytesKV::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(SupplyByBRC20::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(TermsByBRC20Ticker::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(BRC20Balances::CF_NAME, opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(DunesReservedCountersKV::CF_NAME, opts.clone()),
        ];

        let db = OptimisticTransactionDB::open_cf_descriptors(&opts, path.clone(), cfs)
            .map_err(Error::Rocks)?;

        let tx = db.transaction();

        let time_to_open = now.elapsed().as_secs();

        if time_to_open >= 2 * 60 {
            warn!("opened chaindb in {time_to_open} seconds");
        } else {
            info!("opened chaindb in {time_to_open} seconds");
        }

        let notifier = Arc::new(tokio::sync::Notify::new());

        let last_block_hash = BlockByHeightKV::last_entry(&db, &tx)?.map(|(_, DBSerde((h, _)))| h);

        info!("found last block hash: {last_block_hash:?}");

        tx.commit().map_err(Error::Rocks)?;

        // clear MutableKV on reboot
        MutableKV::reset(&db)?;

        Ok(Self {
            db: Arc::new(db),
            notifier,
            next_wal_seq: 1,
            last_block_hash,
            immutable_after_confs,
            network,
            first_rune_height,
            first_inscription_height,
            jubilee_height,
        })
    }

    pub fn is_empty(&self, tx: &Transaction<OptimisticTransactionDB>) -> bool {
        BlockByHeightKV::is_empty(&self.db, tx)
    }

    pub fn cursor(&self) -> Result<Option<(BlockHeight, BlockHash)>, Error> {
        let tx = self.db.transaction();

        let v = BlockByHeightKV::last_entry(&self.db, &tx)?;
        let out = v.map(|(DBInt(hi), DBSerde((ha, _)))| (hi, ha));

        tx.commit().map_err(Error::Rocks)?;

        Ok(out)
    }

    pub fn cfs(&self) -> Vec<Arc<BoundColumnFamily>> {
        let cfs = vec![
            BlockByHeightKV::cf(&self.db),
            MutableKV::cf(&self.db),
            ResolverByHeightKV::cf(&self.db),
            TxoKV::cf(&self.db),
            DuneIdByNameKV::cf(&self.db),
            DuneMintsByIdKV::cf(&self.db),
            DuneTermsByIdKV::cf(&self.db),
            InscriptionCountersKV::cf(&self.db),
            PartialTxidToTxidsKV::cf(&self.db),
            PartialTxidToBytesKV::cf(&self.db),
            SupplyByBRC20::cf(&self.db),
            TermsByBRC20Ticker::cf(&self.db),
            BRC20Balances::cf(&self.db),
            DunesReservedCountersKV::cf(&self.db),
        ];

        cfs
    }

    pub fn compact(&self) -> Result<(), Error> {
        for cf in self.cfs() {
            self.db
                .compact_range_cf(&cf, None::<Vec<u8>>, None::<Vec<u8>>);
        }

        Ok(())
    }

    pub fn flush(&self) -> Result<(), Error> {
        self.db.flush_wal(false).map_err(Error::Rocks)?;

        for cf in self.cfs() {
            self.db.flush_cf(&cf).map_err(Error::Rocks)?;
        }

        Ok(())
    }

    pub fn intersect_options(
        &self,
        genesis_hash: &BlockHash,
    ) -> Result<Vec<(BlockHeight, BlockHash)>, Error> {
        let db_tx = self.db.snapshot();

        let mut out = vec![];

        let last_entry_height =
            BlockByHeightKV::last_entry_snapshot(&self.db, &db_tx)?.map(|(DBInt(x), _)| x);

        let mut indexes = vec![];

        if let Some(tip_height) = last_entry_height {
            let mut step = 1;
            let mut index = tip_height;

            loop {
                indexes.push(index);

                if indexes.len() >= 10 {
                    step *= 2;
                }

                index = index.saturating_sub(step);

                if index == 0 {
                    break;
                }
            }

            for x in indexes {
                let DBSerde((hash, _)) =
                    BlockByHeightKV::get_by_key_snapshot(&self.db, &db_tx, DBInt(x))?
                        .ok_or_else(|| Error::IntersectMissingBlock(x))?;

                out.push((x, hash));
            }
        }

        out.push((0, *genesis_hash));

        Ok(out)
    }

    pub fn destroy(path: impl AsRef<Path>) -> Result<(), Error> {
        DB::destroy(&Options::default(), path).map_err(Error::Rocks)?;

        Ok(())
    }

    pub fn apply_block_with_context(
        &mut self,
        height: BlockHeight,
        block: Block,
        mutable: bool,
        memory_resolver: &mut Option<UtxoResolver>,
    ) -> Result<(), Error> {
        let mut block_bytes = vec![];
        block
            .consensus_encode(&mut block_bytes)
            .map_err(Error::BitcoinEncode)?;

        let hash = block.block_hash(); // TODO: compute only once
        let previous_hash = block.header.prev_blockhash;

        if let Some(h) = self.last_block_hash {
            if previous_hash != h {
                return Err(Error::PreviousHashMismatch {
                    applying: hash,
                    received_prev: previous_hash,
                    expected_prev: Some(h),
                });
            }
        }

        let mut db_tx = self.db.transaction();

        debug!("applying block ({height}, {hash}) (mutable: {mutable})");

        // --- resolve required utxos using utxos produced by the block and the
        // utxo table, process runes and ordinals

        let mut inscription_counters = InscriptionCountersKV::get_by_key(
            &self.db,
            &db_tx,
            DBBytes(INSCRIPTION_COUNTERS_KEY.to_vec()),
        )?
        .map(|DBSerde(x)| x)
        .unwrap_or_default();

        let original_counters = inscription_counters.clone();

        let resolver_result = TxoKV::resolve_and_process(
            memory_resolver,
            &self,
            &mut db_tx,
            height,
            &block,
            &hash,
            self.first_rune_height,
            self.first_inscription_height,
            self.jubilee_height,
            self.network,
            &mut inscription_counters,
        )?;

        let ResolveAndProcessResult {
            mut resolver,
            successful_etches,
            successful_mints,
            tx_ids,
            input_refs,
            stats: resolve_stats,
            valid_reinscriptions,
            brc20_resolver,
            new_inscriptions,
        } = resolver_result;

        // update inscription counters
        InscriptionCountersKV::stage_upsert(
            &self.db,
            DBBytes(INSCRIPTION_COUNTERS_KEY.to_vec()),
            DBSerde(inscription_counters),
            &db_tx,
        )?;

        let mut output_runes_resolver = Vec::new();
        let mut output_inscriptions_resolver = Vec::new();

        // --- insert block bytes into BlockBySlotKV

        BlockByHeightKV::stage_upsert(
            &self.db,
            DBInt(height),
            DBSerde((hash, block_bytes.clone())),
            &mut db_tx,
        )?;

        // --- insert produced txos into TxoKV, and removed consumed txos

        // TODO: skip inserting entries that are deleted in the same batch

        let txo_kv_start = tokio::time::Instant::now();

        for (tx, tx_id) in block.txdata.iter().zip(tx_ids) {
            for (idx, output) in tx.output.iter().enumerate() {
                let mut body = vec![];
                output
                    .consensus_encode(&mut body)
                    .map_err(Error::BitcoinEncode)?;

                let txo_ref = TxoRef(tx_id, idx as u32);

                let entry = resolver
                    .get(&txo_ref)
                    .ok_or(Error::MissingTxo(hash, tx_id, idx as u32))?
                    .clone();

                let (ord_ranges, dunes, inscriptions, unused_brc20_transfers) = (
                    entry.ord_ranges,
                    entry.dunes,
                    entry.inscriptions,
                    entry.unused_brc20_transfers,
                );

                // if the output contains runes, add to the output runes resolver
                if !dunes.is_empty() {
                    output_runes_resolver.push((txo_ref.clone(), dunes.clone()))
                }

                // if the output contains inscriptons, add to the output inscriptions resolver
                if !inscriptions.is_empty() {
                    output_inscriptions_resolver.push((txo_ref.clone(), inscriptions.clone()))
                }

                let txo_body = TxoBody {
                    height,
                    raw: body,
                    ord_ranges,
                    dunes,
                    inscriptions,
                    unused_brc20_transfers,
                };

                TxoKV::stage_upsert(&self.db, DBSerde(txo_ref), DBSerde(txo_body), &mut db_tx)?;
            }

            for input in tx.input.iter().map(|x| x.previous_output) {
                let txo_ref = TxoRef(input.txid, input.vout.into());

                TxoKV::stage_delete(&self.db, DBSerde(txo_ref), &mut db_tx)?;
            }
        }

        let txo_kv_duration = txo_kv_start.elapsed().as_millis();

        // --- insert required txos into ResolverKV

        // we will only store the inputs in the resolver (not outputs)
        resolver.retain(|k, _| input_refs.contains(k));

        let resolver_vec = resolver.clone().into_iter().collect();

        ResolverByHeightKV::stage_upsert(
            &self.db,
            DBInt(height),
            DBSerde((
                hash,
                resolver_vec,
                output_runes_resolver.clone(),
                successful_etches.clone(),
                successful_mints.clone(),
                output_inscriptions_resolver.clone(),
                valid_reinscriptions.clone(),
                original_counters.clone(),
                brc20_resolver.clone(),
                new_inscriptions.clone(),
            )),
            &mut db_tx,
        )?;

        // --- if we are mutable, insert an Apply log entry in the mutableKV

        if mutable {
            let log = Log::Apply(
                height,
                hash,
                block_bytes.clone(),
                resolver,
                output_runes_resolver.into_iter().collect(),
                successful_etches,
                successful_mints,
                output_inscriptions_resolver.into_iter().collect(),
                valid_reinscriptions,
                original_counters,
                brc20_resolver,
                new_inscriptions,
            );

            MutableKV::stage_upsert(&self.db, DBInt(self.next_wal_seq), DBSerde(log), &mut db_tx)?;
        }

        // --- prune MutableKV if necessary

        if mutable {
            if let Some(prune_after) = self.immutable_after_confs {
                if height >= prune_after {
                    let _ = MutableKV::prune(&self.db, &mut db_tx, height - prune_after);
                }
            }
        }

        // --- commit tx

        let commit_start = tokio::time::Instant::now();

        db_tx.commit().map_err(Error::Rocks)?;

        let commit_duration = commit_start.elapsed().as_millis();

        let flush_duration = if height % 5000 == 0 {
            let flush_start = tokio::time::Instant::now();

            self.flush()?;

            Some(flush_start.elapsed().as_millis())
        } else {
            None
        };

        // ---

        info!(
            block=?(height, hash),
            ?resolve_stats,
            mutable,
            commit_duration,
            txo_kv_duration,
            flush_duration,
            memory_bytes=memory_resolver.as_ref().map(|x| x.size()),
            memory_utxos=memory_resolver.as_ref().map(|x| x.len()),
            "applied block with context to chaindb"
        );

        self.next_wal_seq += 1;
        self.last_block_hash = Some(hash);

        self.notifier.notify_waiters();

        Ok(())
    }

    pub fn rollback(
        &mut self,
        rb_height: BlockHeight,
        rb_hash: BlockHash,
        mutable: bool,
    ) -> Result<(), Error> {
        info!("rolling back to ({rb_height}, {rb_hash})");

        let mut db_tx = self.db.transaction();

        let mut next_wal_seq = self.next_wal_seq;

        // --- find points following rollback point

        let mut to_remove = BlockByHeightKV::iter_entries_from(&self.db, &db_tx, DBInt(rb_height));

        let rb_bytes = match to_remove.next() {
            Some(entry) => {
                let (DBInt(found_height), DBSerde((found_hash, found_bytes))) = entry?;

                if rb_height != found_height || rb_hash != found_hash {
                    return Err(Error::InvalidRollbackPoint(Some((
                        found_height,
                        found_hash,
                    ))));
                }

                found_bytes
            }
            None => return Err(Error::InvalidRollbackPoint(None)),
        };

        let to_remove = to_remove.collect::<Vec<_>>();

        // --- for each rollbacked point, starting with most recent...

        for entry in to_remove.into_iter().rev() {
            let (DBInt(height), DBSerde((hash, block_bytes))) = entry?;

            info!("undoing ({height}, {hash})...");

            // --- add Undo action to MutableKV

            if mutable {
                let log = Log::Undo(height, hash, block_bytes.clone());

                MutableKV::stage_upsert(&self.db, DBInt(next_wal_seq), DBSerde(log), &mut db_tx)?;

                next_wal_seq += 1;
            }

            // --- remove entry from BlockBySlotKV

            BlockByHeightKV::stage_delete(&self.db, DBInt(height), &mut db_tx)?;

            // --- remove txos produced by block from TxoKV, reinsert consumed

            let block =
                real_bitcoin::Block::consensus_decode_from_finite_reader(&mut &block_bytes[..])
                    .map_err(|_| Error::Internal("can't decode block when undoing".into()))?;

            let DBSerde((res_hash, resolver, _, _etches, mints, _, _, counters, brc20_resolver, _)) =
                ResolverByHeightKV::get_by_key(&self.db, &mut db_tx, DBInt(height))?
                    .ok_or(Error::MissingResolver(hash))?;

            if res_hash != hash {
                return Err(Error::ResolverHashMismatch(height, hash, res_hash));
            }

            // reset inscription counters
            InscriptionCountersKV::stage_upsert(
                &self.db,
                DBBytes(INSCRIPTION_COUNTERS_KEY.to_vec()),
                DBSerde(counters),
                &db_tx,
            )?;

            // brc20
            // if something was deployed, to undo we need to delete the terms key
            // if something was minted, to undo we need to decrease balance of receiver and decrease supply
            // if a new transfer inscription was made, to undo we need to increase balance of receiver
            // if a transfer was first transferred, we need to decrease the balance of receiver

            // undo actions in reverse order they were applied
            for (_, action) in brc20_resolver.into_iter().rev() {
                match action {
                    BRC20Message::Deploy(tick) => {
                        debug!("deleting brc20 entry for {tick:?}");

                        TermsByBRC20Ticker::stage_delete(&self.db, DBBytes(tick), &db_tx)?;
                    }
                    BRC20Message::Mint(tick, amt, receiver) => {
                        debug!("mint decrease balance and supply {tick:?} {amt:?} {receiver:?}");

                        let supply =
                            SupplyByBRC20::get_by_key(&self.db, &db_tx, DBBytes(tick.clone()))?
                                .map(|DBUInt128(x)| x)
                                .unwrap_or_default();

                        // decrease total minted supply

                        let new_supply = supply - amt;

                        SupplyByBRC20::stage_upsert(
                            &self.db,
                            DBBytes(tick.clone()),
                            DBUInt128(new_supply),
                            &db_tx,
                        )?;

                        // update receiver balance

                        let balance_key = DBSerde(ScriptAndBRC20Kind {
                            script: receiver.to_byte_array(),
                            brc20_ticker: tick,
                        });

                        let old_balance =
                            BRC20Balances::get_by_key(&self.db, &db_tx, balance_key.clone())?
                                .map(|DBUInt128(x)| x)
                                .unwrap_or_default();

                        let new_balance = old_balance - amt;

                        BRC20Balances::stage_upsert(
                            &self.db,
                            balance_key,
                            DBUInt128(new_balance),
                            &db_tx,
                        )?;
                    }
                    // a transfer which occured (an unused transfer was in an input)
                    BRC20Message::Transfer(tick, amt, _, receiver) => {
                        debug!("transfer decrease balance {tick:?} {amt:?} {receiver:?}");

                        let balance_key = DBSerde(ScriptAndBRC20Kind {
                            script: receiver.to_byte_array(),
                            brc20_ticker: tick.to_vec(),
                        });

                        let old_balance =
                            BRC20Balances::get_by_key(&self.db, &db_tx, balance_key.clone())?
                                .map(|DBUInt128(x)| x)
                                .unwrap_or_default();

                        let new_balance = old_balance - amt;

                        BRC20Balances::stage_upsert(
                            &self.db,
                            balance_key,
                            DBUInt128(new_balance),
                            &db_tx,
                        )?;
                    }
                    // a transfer which was initiated/inscription was created
                    BRC20Message::TransferInit(tick, amt, receiver) => {
                        debug!("transferinit increase balance {tick:?} {amt:?} {receiver:?}");

                        let balance_key = DBSerde(ScriptAndBRC20Kind {
                            script: receiver.to_byte_array(),
                            brc20_ticker: tick.to_vec(),
                        });

                        let old_balance =
                            BRC20Balances::get_by_key(&self.db, &db_tx, balance_key.clone())?
                                .map(|DBUInt128(x)| x)
                                .unwrap_or_default();

                        let new_balance = old_balance + amt;

                        BRC20Balances::stage_upsert(
                            &self.db,
                            balance_key,
                            DBUInt128(new_balance),
                            &db_tx,
                        )?;
                    }
                }
            }

            let mut resolver: HashMap<_, _> = resolver.into_iter().collect();

            // undo actions in reverse order
            for (idx, tx) in block.txdata.iter().enumerate().rev() {
                // undo any doge inscription writes
                PartialTxidToTxidsKV::stage_delete(
                    &self.db,
                    DBHash32(tx.txid().to_byte_array()),
                    &db_tx,
                )?;
                PartialTxidToBytesKV::stage_delete(
                    &self.db,
                    DBHash32(tx.txid().to_byte_array()),
                    &db_tx,
                )?;

                // undo any runes actions
                let mut buf = vec![];
                tx.consensus_encode(&mut buf).unwrap();

                if let Some(dunestone) = Dunestone::from_transaction_bytes(buf).unwrap() {
                    let etch_id = DuneId {
                        block: height,
                        tx: idx as u64,
                    };

                    // undo etches
                    if let Some(dune) = dunestone.etching {
                        if let Some(terms) =
                            DuneTermsByIdKV::get_by_key(&self.db, &db_tx, etch_id.clone())?
                        {
                            DuneIdByNameKV::stage_delete(&self.db, DBUInt128(terms.name), &db_tx)?;
                            DuneTermsByIdKV::stage_delete(&self.db, etch_id, &db_tx)?;

                            if dune.dune.is_none() {
                                let DBInt(old_reserved) = DunesReservedCountersKV::get_by_key(
                                    &self.db,
                                    &db_tx,
                                    DBBytes(DUNES_RESERVED_COUNTERS_KEY.to_vec()),
                                )?
                                .unwrap_or(DBInt(0));

                                let new_reserved = old_reserved
                                    .checked_sub(1)
                                    .expect("reserved underflow in rollback");

                                DunesReservedCountersKV::stage_upsert(
                                    &self.db,
                                    DBBytes(DUNES_RESERVED_COUNTERS_KEY.to_vec()),
                                    DBInt(new_reserved),
                                    &db_tx,
                                )?;
                            }
                        }
                    }

                    // undo mints
                    let mut claims = dunestone
                        .edicts
                        .iter()
                        .filter_map(|edict| claim(edict.id))
                        .collect::<Vec<u128>>();
                    claims.sort();
                    claims.dedup();

                    for (claim_idx, id) in claims.into_iter().enumerate() {
                        if let Ok(key) = dunes::dune_id::DuneId::try_from(id) {
                            if mints.contains(&(idx as u32, claim_idx as u16)) {
                                let mint_id = DuneId {
                                    block: key.height,
                                    tx: key.index as u64,
                                };

                                let DBUInt128(old_mints) =
                                    DuneMintsByIdKV::get_by_key(&self.db, &db_tx, mint_id.clone())?
                                        .unwrap_or(DBUInt128(0));

                                let new_mints = old_mints
                                    .checked_sub(1)
                                    .expect("mints underflow in rollback");

                                DuneMintsByIdKV::stage_upsert(
                                    &self.db,
                                    mint_id,
                                    DBUInt128(new_mints),
                                    &db_tx,
                                )?;
                            }
                        }
                    }
                }

                // dont try insert coinbase input
                if idx != 0 {
                    for input in tx.input.iter().map(|x| x.previous_output) {
                        let txid = Txid::from_byte_array(input.txid.to_byte_array());

                        let txo_ref = TxoRef(txid, input.vout);

                        let body = resolver
                            .remove(&txo_ref)
                            .ok_or(Error::MissingTxo(hash, txid, input.vout))?;

                        TxoKV::stage_upsert(&self.db, DBSerde(txo_ref), DBSerde(body), &mut db_tx)?;
                    }
                }

                let txid = Txid::from_byte_array(tx.txid().to_byte_array());

                for (idx, _) in tx.output.iter().enumerate() {
                    TxoKV::stage_delete(&self.db, DBSerde(TxoRef(txid, idx as u32)), &mut db_tx)?
                }
            }

            // --- remove ResolverKV entry for block

            ResolverByHeightKV::stage_delete(&self.db, DBInt(height), &mut db_tx)?;
        }

        // --- add Mark action to MutableKV for rollback point

        if mutable {
            let log = Log::Mark(rb_height, rb_hash, rb_bytes);

            MutableKV::stage_upsert(&self.db, DBInt(next_wal_seq), DBSerde(log), &mut db_tx)?;

            next_wal_seq += 1;
        }

        // ---

        db_tx.commit().map_err(Error::Rocks)?;

        info!("rolled back to ({rb_height}, {rb_hash})");

        // ---

        self.next_wal_seq = next_wal_seq;
        self.last_block_hash = Some(rb_hash);

        self.notifier.notify_waiters();

        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewInscriptionInfo {
    pub tx_hash: [u8; 32], // Hash of reveal tx
    pub id: InscriptionId,
    pub number: u64,
    pub body: Vec<u8>,      // content body raw data
    pub body_type: Vec<u8>, // content body type
}
