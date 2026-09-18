use bitcoin::{hashes::Hash, BlockHash};
use builder::PREFIX_DATA;
use gasket::{
    error::AsWorkError,
    runtime::{spawn_stage, ScheduleResult, WorkSchedule},
};
use itertools::Itertools;
use redis::Commands;
use serde::Deserialize;
use std::time::Duration;
use std::{collections::HashMap, u64};
use tikv_client::{
    CommitTTLParameters, Error, Key, KvPair, Timestamp, TimestampExt, Transaction,
    TransactionClient, TransactionOptions,
};
use timbre_xbt::{
    rollback::{rollback_metadata_key_range, MetadataKey},
    Decode,
};
use tracing::error;
use tracing::{debug, info, warn};

use crate::{
    bootstrap,
    crosscut::{self, Point},
    model::{
        self,
        StorageAction::{self, *},
        StorageActionPayload,
    },
    prelude::AppliesPolicy,
    reducers::{
        balances_by_brc20, balances_by_rune_id, brc20_balances_by_script_hash,
        brc20_terms_by_ticker, content_by_inscription_id, etching_by_rune_id,
        inscription_utxos_by_script_hash, inscriptions_by_script_hash, inscriptions_by_utxo,
        mints_by_rune_id, rune_balances_by_script_hash, rune_id_by_rune_name, runes_by_utxo,
        sat_balance_by_script_hash, script_by_script_hash, script_hash_by_address_payload_hash,
        transfer_inscriptions_by_script_hash, txs_by_script_hash, utxos_by_rune_id,
        utxos_by_script_hash, IncrOrDecr, ReducerOutput, UtxoAction,
    },
    rollback::{buffer::RollbackBuffer, PersistentBufferValue},
    storage::RedisEntry,
};

use timbre_xbt::*;

type InputPort = gasket::messaging::tokio::InputPort<model::StorageActionPayload>;

type StorageActions = Vec<StorageAction>;

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    /// TiKV PD
    pub connection_params: String,
    /// Redis address
    pub redis_address: String,
    /// Polyphony dataplane identifier
    pub dataplane_id: u8,
    /// Which network is this dataplane for
    pub network: String,
    /// Polyphony instance identifier so multiple Polyphony can use the same DB
    pub instance_id: u8,
    /// The max number of blocks to batch into a single DB transaction during sync
    pub sync_batch_size: Option<usize>,
    /// TiKV config value `raft-entry-max-size` in bytes
    pub tikv_raft_entry_max_size: Option<usize>,
    /// TiKV use pessimistic mode for txs (default false/optimistic mode)
    pub tikv_pessimistic_mode: Option<bool>,
    /// TiKV commit min TTL in millis (default 3000)
    pub tikv_commit_min_ttl: Option<u64>,
    /// TiKV commit max TTL in millis (default 20000)
    pub tikv_commit_max_ttl: Option<u64>,
    /// TiKV commit batch size (default 16kb)
    pub tikv_commit_batch_size: Option<u64>,
    /// TiKV commit TTL multiplier (default 6000.0)
    pub tikv_commit_ttl_factor: Option<f64>,
    /// TiKV commit TTL multiplier (default false)
    pub tikv_cleanup_locks: Option<bool>,
    /// True if we want to display warnings if attempting to delete non-existent keys
    pub key_delete_warnings: Option<bool>,
    /// Advertise in the instance registry immediately rather than waiting to
    /// reach the mutable window (useful for development)
    pub advertise_immediately: Option<bool>,
}

impl Config {
    pub fn bootstrapper(
        self,
        policy: &crosscut::policies::RuntimePolicy,
        instance_names: Vec<String>,
    ) -> Bootstrapper {
        Bootstrapper {
            config: self,
            policy: policy.clone(),
            input: Default::default(),
            instance_names,
        }
    }

    fn build_tx_opts(&self) -> TransactionOptions {
        let mut tx_opts = if self.tikv_pessimistic_mode.unwrap_or(false) {
            TransactionOptions::new_pessimistic()
        } else {
            TransactionOptions::new_optimistic()
        };

        let mut ttl_parameters = CommitTTLParameters::default();

        if let Some(x) = self.tikv_commit_min_ttl {
            ttl_parameters = ttl_parameters.min_ttl(x)
        }

        if let Some(x) = self.tikv_commit_max_ttl {
            ttl_parameters = ttl_parameters.max_ttl(x)
        }

        if let Some(x) = self.tikv_commit_batch_size {
            ttl_parameters = ttl_parameters.txn_commit_batch_size(x)
        }

        if let Some(x) = self.tikv_commit_ttl_factor {
            ttl_parameters = ttl_parameters.ttl_factor(x)
        }

        tx_opts = tx_opts.ttl_parameters(ttl_parameters);

        tx_opts = tx_opts.drop_check(tikv_client::CheckLevel::Warn);

        tx_opts
    }
}

pub struct Bootstrapper {
    config: Config,
    policy: crosscut::policies::RuntimePolicy,
    input: InputPort,
    instance_names: Vec<String>,
}

impl Bootstrapper {
    pub fn borrow_input_port(&mut self) -> &'_ mut InputPort {
        &mut self.input
    }

    pub fn build_cursor(&self) -> Cursor {
        Cursor {
            config: self.config.clone(),
        }
    }

    pub fn spawn_stages(
        self,
        pipeline: &mut bootstrap::Pipeline,
        intersect: Option<Point>,
        buf: Option<Vec<PersistentBufferValue>>,
        buffer_size: usize,
        timeout: u64,
    ) {
        let mut rollback_buffer: RollbackBuffer<StorageActions> = RollbackBuffer::new(buffer_size);

        if let Some(persistent_buf) = buf {
            for entry in persistent_buf {
                rollback_buffer.add_block(entry.point.into(), entry.inverse_actions)
            }

            tracing::debug!(
                "populated memory rollback buffer for storage stage: {:?}",
                rollback_buffer
            )
        } else {
            tracing::debug!("no persistent rollback buffer found to populate memory buf")
        }

        let worker = Worker {
            config: self.config.clone(),
            policy: self.policy.clone(),
            connection: None,
            redis_connection: None,
            input: self.input,
            ops_count: Default::default(),
            key_encoder: Prefix::new(self.config.dataplane_id, self.config.instance_id),
            sync_batcher: SyncBatcher::new(
                self.config.sync_batch_size.unwrap_or(1),
                self.config.tikv_raft_entry_max_size.unwrap_or(4000000),
            ),
            tx_options: self.config.build_tx_opts(),
            rollback_buffer,
            work_unit_2pc: None,
            mutable: false,
            last_processed: intersect,
            buffer_size,
            instance_names: self.instance_names,
            registered: false,
        };

        info!(
            "using dataplane id {} with instance id {} and buffer size {buffer_size}",
            self.config.dataplane_id, self.config.instance_id
        );

        pipeline.register_stage(spawn_stage(
            worker,
            gasket::runtime::Policy {
                tick_timeout: Some(Duration::from_secs(timeout)),
                bootstrap_retry: gasket::retries::Policy {
                    max_retries: 20,
                    backoff_unit: Duration::from_secs(1),
                    backoff_factor: 2,
                    max_backoff: Duration::from_secs(60),
                },
                ..Default::default()
            },
            Some("tikv"),
        ));
    }
}

pub struct Cursor {
    config: Config,
}

impl Cursor {
    pub async fn last_point(&mut self) -> Result<Option<Point>, crate::Error> {
        let connection = TransactionClient::new(vec![self.config.connection_params.clone()])
            .await
            .map_err(crate::Error::storage)?;

        let encoder = Prefix::new(self.config.dataplane_id, self.config.instance_id);

        let mut snapshot = connection.snapshot(
            connection
                .current_timestamp()
                .await
                .map_err(crate::Error::storage)?,
            TransactionOptions::new_optimistic().drop_check(tikv_client::CheckLevel::Warn),
        );

        let raw = snapshot
            .get(encoder.cursor())
            .await
            .map_err(crate::Error::storage)?;

        let point = match raw {
            Some(x) => {
                let ((height, hash), _) = <_>::decode(&x).unwrap();
                Some(Point {
                    height,
                    hash: BlockHash::from_byte_array(hash),
                })
            }
            None => None,
        };

        Ok(point)
    }

    /// Fetch the persistent buffer from storage, if it exists, to bootstrap the
    /// stage rollback buffers on start-up.
    ///
    /// TODO: Refactor...
    pub async fn fetch_persistent_buffer(
        &mut self,
    ) -> Result<Option<Vec<PersistentBufferValue>>, crate::Error> {
        let connection = TransactionClient::new(vec![self.config.connection_params.clone()])
            .await
            .map_err(crate::Error::storage)?;

        let mut persistent_buf: Vec<PersistentBufferValue> = Vec::new();

        let mut snapshot = connection.snapshot(
            connection
                .current_timestamp()
                .await
                .map_err(crate::Error::storage)?,
            TransactionOptions::new_optimistic().drop_check(tikv_client::CheckLevel::Warn),
        );

        // scan all metadata keys, for each point create a vec of inverse
        // storage ops

        let range = rollback_metadata_key_range(
            &Namespace::new(self.config.dataplane_id, self.config.instance_id),
            None::<u64>,
            None::<u64>,
        );

        let mut range = range.0..range.1;

        let mut current_point = Point {
            height: 0,
            hash: BlockHash::from_byte_array([0; 32]),
        };

        let mut current_point_height = 0;

        let mut actions = Vec::new();

        let mut scan_size = 2000;
        let mut kvs = Vec::new();

        loop {
            match snapshot.scan(range.clone(), scan_size).await {
                Ok(kvs_iter) => {
                    let kvs_vec: Vec<KvPair> = kvs_iter.collect();

                    kvs.extend(kvs_vec.clone());

                    if kvs_vec.is_empty() {
                        break;
                    }

                    let mut last_key =
                        Into::<Vec<u8>>::into(kvs_vec.last().unwrap().clone().into_key());

                    debug!(
                        "scanned {} rb metadata, {} total, last key {}",
                        kvs_vec.len(),
                        kvs.len(),
                        hex::encode(&last_key)
                    );

                    last_key.push(0);
                    range = last_key..range.end;
                }
                Err(Error::Grpc(e))
                    if e.to_string().contains("Received message larger than max") =>
                {
                    warn!("halving scan size because got error {e:?}");
                    scan_size /= 2
                }
                Err(e) => {
                    error!("error when trying to fetch persistent buffer {e:?}");
                    return Err(e).map_err(crate::Error::storage);
                }
            }
        }

        for KvPair(k, v) in kvs.into_iter() {
            let k: Vec<u8> = k.into();
            let k = k.get(4..).unwrap();
            let (metadata_key, _) = MetadataKey::decode(&k).unwrap();
            debug!("rollback metadata key found: {metadata_key:?}");

            // we are now looking at metadata for a new point, push info for
            // previous point to buffer
            if metadata_key.height != current_point_height {
                // don't include the init point
                if current_point_height != 0 {
                    let bufval = PersistentBufferValue {
                        point: current_point.clone(),
                        inverse_actions: actions.clone(),
                    };

                    debug!("finished parsing point rb metadata: {bufval:?}");

                    persistent_buf.push(bufval);
                }

                // reset accumulators and set point to next
                current_point = Point {
                    height: metadata_key.height,
                    hash: BlockHash::from_byte_array(metadata_key.hash),
                };

                current_point_height = current_point.height;
                actions.clear();
            }

            if v.is_empty() {
                // empty value means we need to delete the key
                actions.push(StorageAction::Delete(metadata_key.modified_key))
            } else {
                // non-empty value means we need to create or overwrite key with previous value
                actions.push(StorageAction::Set(metadata_key.modified_key, v))
            }
        }

        // we have finished building the buf val for the most recent point as we
        // have exhausted all the rb metadata keys, push the final point. don't push
        // if it is the origin point though.
        if current_point_height != 0 {
            let bufval = PersistentBufferValue {
                point: current_point.clone(),
                inverse_actions: actions.clone(),
            };

            debug!("finished parsing final point rb metadata: {bufval:?}");

            persistent_buf.push(bufval);
        }

        if persistent_buf.is_empty() {
            info!("no persistent buffer");

            Ok(None)
        } else {
            info!("persistent buffer found ({})", persistent_buf.len());

            Ok(Some(persistent_buf))
        }
    }
}

#[derive(Clone)]
struct SyncBatcher {
    batch_actions: HashMap<model::Key, StorageAction>,
    max_len: usize,
    current_len: usize,
    max_bytes: usize,
    estimated_bytes: usize, // ~0.6 of resulting raft msg
    merge_counter: usize,
    merge_counter_incr: (usize, usize),
    merge_counter_set: (usize, usize),
}

impl SyncBatcher {
    fn new(max_len: usize, tikv_max_raft_entry_bytes: usize) -> Self {
        SyncBatcher {
            batch_actions: HashMap::new(),
            max_len,
            current_len: 0,
            max_bytes: tikv_max_raft_entry_bytes,
            estimated_bytes: 0,
            merge_counter: 0,
            merge_counter_incr: (0, 0),
            merge_counter_set: (0, 0),
        }
    }

    fn push_actions(&mut self, actions: StorageActions) {
        for action in actions {
            self.estimated_bytes += action.size();

            match action {
                StorageAction::Set(_, _) | Delete(_) => self.merge_counter_set.1 += 1,
                _ => self.merge_counter_incr.1 += 1,
            }
            if let Some(prev) = self.batch_actions.get_mut(action.key()) {
                self.merge_counter += 1;
                match action {
                    StorageAction::Set(_, _) | Delete(_) => self.merge_counter_set.0 += 1,
                    _ => self.merge_counter_incr.0 += 1,
                }

                prev.merge(action);
            } else {
                self.batch_actions.insert(action.key().clone(), action);
            }
        }

        self.current_len += 1;
    }

    fn wipe(&mut self) {
        self.current_len = 0;
        self.batch_actions = HashMap::new();
        self.estimated_bytes = 0;
        self.merge_counter = 0;
        self.merge_counter_incr = (0, 0);
        self.merge_counter_set = (0, 0);
    }

    fn get_batch_actions(&self) -> StorageActions {
        self.batch_actions.values().cloned().collect()
    }

    /// If the estimated bytes size of the batcher with the addition of the
    /// incoming actions will be greater than half of the TiKV
    /// `max-raft-entry-size` limit then we should flush the batch now, and
    /// start a new batch with the incoming actions.
    fn should_flush(&self, next_actions: &StorageActions) -> bool {
        let next_size: usize = next_actions.iter().map(|x| x.size()).sum();

        if (self.estimated_bytes + next_size) * 2 > self.max_bytes {
            debug!(
                "should flush due to estimated batch bytes: [curr: {}, next: {}, max: {}]",
                self.estimated_bytes, next_size, self.max_bytes
            );

            true
        } else if self.current_len >= self.max_len {
            debug!("should flush due to reaching max blocks per batch");

            true
        } else {
            false
        }
    }

    fn is_empty(&self) -> bool {
        self.batch_actions.is_empty()
    }
}

pub struct Worker {
    config: Config,
    policy: crosscut::policies::RuntimePolicy,
    connection: Option<tikv_client::TransactionClient>,
    redis_connection: Option<redis::Client>,
    ops_count: gasket::metrics::Counter,
    input: InputPort,
    key_encoder: Prefix,
    sync_batcher: SyncBatcher,
    tx_options: TransactionOptions,
    rollback_buffer: RollbackBuffer<StorageActions>,
    mutable: bool,
    work_unit_2pc: Option<StorageActionPayload>,
    last_processed: Option<Point>,
    buffer_size: usize,
    instance_names: Vec<String>,
    registered: bool,
}

impl Worker {
    /// The sorted-set key holding this instance's cursor entries, as consumed
    /// by the GC safepoint manager and the admin CLI.
    fn timestamps_key(&self) -> String {
        format!(
            "tikv-timestamps:{}:{}",
            self.config.dataplane_id, self.config.instance_id
        )
    }

    /// Record this instance's timestamps key in the global key index so the
    /// GC safepoint manager can discover it.
    fn index_timestamps_key(&mut self, ts_physical: i64) -> Result<(), gasket::error::Error> {
        let key = self.timestamps_key();
        let _: redis::Value = self
            .redis_connection
            .as_mut()
            .unwrap()
            .zadd("tikv-timestamps-keys", key, ts_physical)
            .or_restart()?;

        Ok(())
    }

    /// Register this instance in the per-instance-group registries once it has
    /// reached the mutable window at the chain tip. A backfilling instance is
    /// invisible to the API layer until then.
    fn maybe_advertise(&mut self, height: u64) -> Result<(), gasket::error::Error> {
        if self.registered {
            return self.update_registry_scores(height);
        }

        if !(self.mutable || self.config.advertise_immediately.unwrap_or(false)) {
            return Ok(());
        }

        self.update_registry_scores(height)?;
        self.registered = true;
        info!("advertising instance in the registry (mutable window reached or advertise_immediately set)");
        Ok(())
    }

    fn update_registry_scores(&mut self, height: u64) -> Result<(), gasket::error::Error> {
        let member = [self.config.dataplane_id, self.config.instance_id];
        let network = self.config.network.to_lowercase();

        for name in self.instance_names.clone() {
            let key = format!("dogecoin:{}:{}:scores", network, name);
            let _: redis::Value = self
                .redis_connection
                .as_mut()
                .unwrap()
                .zadd(key, &member[..], height)
                .or_restart()?;
        }

        Ok(())
    }
}

impl Worker {
    /// Start an TiKV database transaction
    async fn begin_tikv_transaction(&mut self) -> Result<Transaction, tikv_client::Error> {
        let txn = self
            .connection
            .as_mut()
            .unwrap()
            .begin_with_options(self.tx_options.clone())
            .await?;

        let txn_start_ts = txn.start_timestamp();

        debug!(
            "started a tikv transaction at {txn_start_ts:?} ({})",
            txn_start_ts.version()
        );

        Ok(txn)
    }

    /// Clean up any TiKV locks on data keys for this instance
    async fn cleanup_tikv_locks(
        &mut self,
        safepoint: &Timestamp,
        just_data: bool,
    ) -> Result<usize, tikv_client::Error> {
        // range will be all reducer data keys for this instance
        let namespace = self.key_encoder.namespace();

        let range = if just_data {
            // create range for just data keys relating to this instance
            let instance_data_key_range_start = [namespace.encode(), vec![PREFIX_DATA]].concat();

            let instance_data_key_range_end = [namespace.encode(), vec![PREFIX_DATA + 1]].concat();

            instance_data_key_range_start..instance_data_key_range_end
        } else {
            // create range for all keys relating to this instance
            let dp = namespace.dataplane_id;
            let instance = namespace.instance_id;

            if instance != 255 {
                vec![dp, instance]..vec![dp, instance + 1]
            } else if dp != 255 {
                vec![dp, instance]..vec![dp + 1]
            } else {
                // instance and dp are 255 (empty vec means upper unbounded)
                vec![dp, instance]..vec![]
            }
        };

        let resolved_locks = self
            .connection
            .as_mut()
            .unwrap()
            .legacy_cleanup_locks(range.into(), safepoint)
            .await?;

        if resolved_locks > 0 {
            debug!("cleaned up {resolved_locks} tikv locks")
        };

        Ok(resolved_locks)
    }

    /// Convert a ReducerOutput to a StorageAction, by specifying what key and values
    /// should be written to, or which keys should be deleted from, the database. This
    /// means the key and value encoding logic for a ReducerOutput can be different
    /// for different storage backends.
    fn reducer_output_to_storage_ops(&self, output: ReducerOutput) -> Vec<StorageAction> {
        match output {
            ReducerOutput::BalancesByBrc20(balances_by_brc20::Output {
                brc_ticker,
                script_hash,
                total_delta,
            }) => {
                let key = timbre_xbt::reducers::balances_by_brc20::Key {
                    ticker: brc_ticker.try_into().unwrap(),
                    script_hash,
                };

                let total_balance_key = self.key_encoder.data(&Reducer::BalancesByBrc20, &key);

                match total_delta {
                    IncrOrDecr::Increment(x) => {
                        vec![StorageAction::IncrementU128(total_balance_key, x)]
                    }
                    IncrOrDecr::Decrement(x) => {
                        vec![StorageAction::DecrementU128(total_balance_key, x)]
                    }
                }
            }
            ReducerOutput::BalancesByRuneId(balances_by_rune_id::Output {
                rune_id,
                script_hash,
                action,
            }) => {
                let key = timbre_xbt::reducers::balances_by_rune_id::Key {
                    rune_id: (rune_id.0.into(), rune_id.1.into()),
                    script_hash,
                };

                let balance_key = self.key_encoder.data(&Reducer::BalancesByRuneId, &key);

                match action {
                    IncrOrDecr::Increment(x) => {
                        vec![StorageAction::IncrementU128(balance_key, x)]
                    }
                    IncrOrDecr::Decrement(x) => {
                        vec![StorageAction::DecrementU128(balance_key, x)]
                    }
                }
            }
            ReducerOutput::Brc20BalancesByScriptHash(brc20_balances_by_script_hash::Output {
                script_hash,
                brc_ticker,
                total_delta,
                available_delta,
            }) => {
                let key = timbre_xbt::reducers::brc20_balances_by_script_hash::Key {
                    script_hash,
                    ticker: brc_ticker.try_into().unwrap(),
                };

                let mut actions = vec![];

                let total_balance_key = self
                    .key_encoder
                    .data(&Reducer::Brc20TotalBalanceByScriptHash, &key);

                match total_delta {
                    IncrOrDecr::Increment(x) => {
                        actions.push(StorageAction::IncrementU128(total_balance_key, x))
                    }
                    IncrOrDecr::Decrement(x) => {
                        actions.push(StorageAction::DecrementU128NoDelete(total_balance_key, x))
                    }
                }

                let available_balance_key = self
                    .key_encoder
                    .data(&Reducer::Brc20AvailableBalanceByScriptHash, &key);

                match available_delta {
                    IncrOrDecr::Increment(x) => {
                        actions.push(StorageAction::IncrementU128(available_balance_key, x))
                    }
                    IncrOrDecr::Decrement(x) => actions.push(StorageAction::DecrementU128NoDelete(
                        available_balance_key,
                        x,
                    )),
                }

                actions
            }
            ReducerOutput::Brc20TermsByTicker(brc20_terms_by_ticker::Output {
                ticker,
                max,
                limit,
                dec,
                self_mint,
                deploy_id,
            }) => {
                let key = timbre_xbt::reducers::brc20_terms_by_ticker::Key {
                    ticker: ticker.try_into().unwrap(),
                };

                let encoded_key = self.key_encoder.data(&Reducer::Brc20TermsByTicker, &key); // TODO: implicit reducer tag byte

                let value = timbre_xbt::reducers::brc20_terms_by_ticker::Value {
                    max,
                    limit,
                    dec,
                    self_mint,
                    deploy_id,
                };

                vec![StorageAction::Set(encoded_key, value.encode())]
            }
            ReducerOutput::EtchingByRuneId(etching_by_rune_id::Output {
                rune_id,
                tx_hash,
                etching,
                cenotaph,
            }) => {
                let key = timbre_xbt::reducers::etching_by_rune_id::Key { rune_id };

                let encoded_key = self.key_encoder.data(&Reducer::EtchingByRuneId, &key); // TODO: implicit reducer tag byte

                let value = timbre_xbt::reducers::etching_by_rune_id::Value {
                    tx_hash,
                    name: etching.name,
                    spacers: etching.spacers,
                    symbol: etching.symbol,
                    divisibility: etching.divisibility,
                    premine: None, // TODO: include?
                    max_mint_txs: etching.max_mint_txs,
                    amount_per_mint: etching.amount_per_mint,
                    start_height: etching.start_height,
                    end_height: etching.end_height,
                    start_offset: etching.start_offset,
                    end_offset: etching.end_offset,
                    turbo: etching.turbo,
                    cenotaph,
                };

                vec![StorageAction::Set(encoded_key, value.encode())]
            }
            ReducerOutput::ContentByInscriptionId(content_by_inscription_id::Output {
                inscription_id,
                created_at,
                inscription_num,
                content_type,
                content_body,
            }) => {
                let key = timbre_xbt::reducers::content_by_inscription_id::Key { inscription_id };
                let encoded_key = self
                    .key_encoder
                    .data(&Reducer::ContentByInscriptionId, &key);
                let value = timbre_xbt::reducers::content_by_inscription_id::Value {
                    created_at,
                    inscription_num,
                    content_type,
                    content_body,
                };
                vec![StorageAction::Set(encoded_key, value.encode())]
            }
            ReducerOutput::InscriptionUtxosByScriptHash(
                inscription_utxos_by_script_hash::Output {
                    script_hash,
                    height,
                    utxo_hash,
                    utxo_index,
                    action,
                },
            ) => {
                let key = timbre_xbt::reducers::inscription_utxos_by_script_hash::Key {
                    script_hash,
                    height: height.into(),
                    utxo_hash,
                    utxo_index: utxo_index.into(),
                };

                let encoded_key = self
                    .key_encoder
                    .data(&Reducer::InscriptionUtxosByScriptHash, &key); // TODO: implicit reducer tag byte

                match action {
                    UtxoAction::Consumed => vec![StorageAction::Delete(encoded_key)],
                    UtxoAction::Produced((satoshis, inscriptions)) => {
                        let inscriptions: Vec<(VarUInt, ([u8; 32], VarUInt))> = inscriptions
                            .into_iter()
                            .map(|(offset, (reveal_tx_hash, inscription_index))| {
                                (offset.into(), (reveal_tx_hash, inscription_index.into()))
                            })
                            .collect();
                        let value = timbre_xbt::reducers::inscription_utxos_by_script_hash::Value {
                            satoshis: satoshis.into(),
                            inscriptions: inscriptions.into(),
                        };

                        vec![StorageAction::Set(encoded_key, value.encode())]
                    }
                }
            }
            ReducerOutput::InscriptionsByScriptHash(inscriptions_by_script_hash::Output {
                script_hash,
                inscription_id,
                action,
            }) => {
                let key = timbre_xbt::reducers::inscriptions_by_script_hash::Key {
                    script_hash,
                    inscription_id,
                };

                let encoded_key = self
                    .key_encoder
                    .data(&Reducer::InscriptionsByScriptHash, &key);
                match action {
                    UtxoAction::Consumed => vec![StorageAction::Delete(encoded_key)],
                    UtxoAction::Produced((height, (utxo_hash, utxo_index), offset)) => {
                        let value = timbre_xbt::reducers::inscriptions_by_script_hash::Value {
                            height,
                            utxo_hash,
                            utxo_index,
                            offset,
                        };
                        vec![StorageAction::Set(encoded_key, value.encode())]
                    }
                }
            }
            ReducerOutput::InscriptionsByUtxo(inscriptions_by_utxo::Output {
                utxo_hash,
                utxo_index,
                action,
            }) => {
                let key = timbre_xbt::reducers::inscriptions_by_utxo::Key {
                    utxo_hash,
                    utxo_index,
                };

                let encoded_key = self.key_encoder.data(&Reducer::InscriptionsByUtxo, &key); // TODO: implicit reducer tag byte

                match action {
                    UtxoAction::Consumed => vec![StorageAction::Delete(encoded_key)],
                    UtxoAction::Produced(inscriptions) => {
                        let value = timbre_xbt::reducers::inscriptions_by_utxo::Value {
                            inscriptions: inscriptions.into_iter().collect(),
                        };

                        vec![StorageAction::Set(encoded_key, value.encode())]
                    }
                }
            }
            ReducerOutput::MintsByRuneId(mints_by_rune_id::Output { rune_id }) => {
                let key = timbre_xbt::reducers::mints_by_rune_id::Key { rune_id };

                let encoded_key = self.key_encoder.data(&Reducer::MintsByRuneId, &key); // TODO: implicit reducer tag byte

                // TODO: u64?
                vec![StorageAction::IncrementU128(encoded_key, 1)]
            }
            ReducerOutput::RuneBalancesByScriptHash(rune_balances_by_script_hash::Output {
                script_hash,
                rune_id,
                delta,
            }) => {
                let key = timbre_xbt::reducers::rune_balances_by_script_hash::Key {
                    script_hash,
                    rune_id,
                };

                let balance_key = self
                    .key_encoder
                    .data(&Reducer::RuneBalancesByScriptHash, &key);

                match delta {
                    IncrOrDecr::Increment(x) => {
                        vec![StorageAction::IncrementU128(balance_key, x)]
                    }
                    IncrOrDecr::Decrement(x) => {
                        vec![StorageAction::DecrementU128(balance_key, x)]
                    }
                }
            }
            ReducerOutput::RuneIdByRuneName(rune_id_by_rune_name::Output {
                rune_id,
                rune_name,
            }) => {
                let key = timbre_xbt::reducers::rune_id_by_rune_name::Key { rune_name };

                let encoded_key = self.key_encoder.data(&Reducer::RuneIdByRuneName, &key); // TODO: implicit reducer tag byte

                let value = timbre_xbt::reducers::rune_id_by_rune_name::Value { rune_id };

                vec![StorageAction::Set(encoded_key, value.encode())]
            }
            ReducerOutput::RunesByUtxo(runes_by_utxo::Output {
                utxo_hash,
                utxo_index,
                action,
            }) => {
                let key = timbre_xbt::reducers::runes_by_utxo::Key {
                    utxo_hash,
                    utxo_index,
                };

                let encoded_key = self.key_encoder.data(&Reducer::RunesByUtxo, &key); // TODO: implicit reducer tag byte

                match action {
                    UtxoAction::Consumed => vec![StorageAction::Delete(encoded_key)],
                    UtxoAction::Produced(runes) => {
                        let value = timbre_xbt::reducers::runes_by_utxo::Value {
                            runes: runes.into_iter().collect(),
                        };

                        vec![StorageAction::Set(encoded_key, value.encode())]
                    }
                }
            }
            ReducerOutput::SatBalanceByScriptHash(sat_balance_by_script_hash::Output {
                script_hash,
                delta,
            }) => {
                let key = timbre_xbt::reducers::sat_balance_by_script_hash::Key { script_hash };

                let encoded_key = self
                    .key_encoder
                    .data(&Reducer::SatBalanceByScriptHash, &key);

                match delta {
                    IncrOrDecr::Increment(x) => vec![StorageAction::IncrementU64(encoded_key, x)],
                    IncrOrDecr::Decrement(x) => vec![StorageAction::DecrementU64(encoded_key, x)],
                }
            }
            ReducerOutput::ScriptByScriptHash(script_by_script_hash::Output {
                script_hash,
                script,
            }) => {
                let key = timbre_xbt::reducers::script_by_script_hash::Key { script_hash };

                let encoded_key = self.key_encoder.data(&Reducer::ScriptByScriptHash, &key); // TODO: implicit reducer tag byte

                let value = timbre_xbt::reducers::script_by_script_hash::Value { script };

                vec![StorageAction::Set(encoded_key, value.encode())]
            }
            ReducerOutput::ScriptHashByAddressPayloadHash(
                script_hash_by_address_payload_hash::Output {
                    payload_hash,
                    script_hash,
                },
            ) => {
                let key =
                    timbre_xbt::reducers::script_hash_by_address_payload_hash::Key { payload_hash };

                let encoded_key = self
                    .key_encoder
                    .data(&Reducer::ScriptHashByAddressPayloadHash, &key); // TODO: implicit reducer tag byte

                let value = timbre_xbt::reducers::script_hash_by_address_payload_hash::Value {
                    script_hash,
                };

                vec![StorageAction::Set(encoded_key, value.encode())]
            }
            ReducerOutput::TransferInscriptionsByScriptHash(
                transfer_inscriptions_by_script_hash::Output {
                    script_hash,
                    ticker,
                    inscription_id,
                    action,
                },
            ) => {
                let key = timbre_xbt::reducers::transfer_inscriptions_by_script_hash::Key {
                    script_hash,
                    ticker: timbre_xbt::ShortByteString(ticker),
                    inscription_id,
                };

                let encoded_key = self
                    .key_encoder
                    .data(&Reducer::TransferInscriptionsByScriptHash, &key);

                match action {
                    UtxoAction::Consumed => vec![StorageAction::Delete(encoded_key)],
                    UtxoAction::Produced((
                        token_amount,
                        sat_amount,
                        (utxo_hash, utxo_index),
                        offset,
                        block_height,
                    )) => {
                        let value =
                            timbre_xbt::reducers::transfer_inscriptions_by_script_hash::Value {
                                token_amount,
                                sat_amount,
                                utxo_hash,
                                utxo_index,
                                offset,
                                block_height,
                            };

                        vec![StorageAction::Set(encoded_key, value.encode())]
                    }
                }
            }
            ReducerOutput::TxsByScriptHash(txs_by_script_hash::Output {
                script_hash,
                height,
                tx_hash,
                tx_block_index,
                input,
                output,
            }) => {
                let key = timbre_xbt::reducers::txs_by_script_hash::Key {
                    script_hash,
                    height,
                    tx_hash,
                    blk_index: tx_block_index,
                };

                let encoded_key = self.key_encoder.data(&Reducer::TxsByScriptHash, &key);

                let value =
                    timbre_xbt::reducers::txs_by_script_hash::Value { input, output }.encode();

                vec![StorageAction::Set(encoded_key, value)]
            }
            ReducerOutput::UtxosByRuneId(utxos_by_rune_id::Output {
                rune_id,
                height,
                utxo_hash,
                utxo_index,
                action,
            }) => {
                let key = timbre_xbt::reducers::utxos_by_rune_id::Key {
                    rune_id,
                    height,
                    utxo_hash,
                    utxo_index,
                };

                let encoded_key = self.key_encoder.data(&Reducer::UtxosByRuneId, &key);

                match action {
                    UtxoAction::Consumed => vec![StorageAction::Delete(encoded_key)],
                    UtxoAction::Produced((script_hash, rune_quantity, satoshis)) => {
                        let value = timbre_xbt::reducers::utxos_by_rune_id::Value {
                            script_hash,
                            rune_quantity,
                            satoshis,
                        };

                        vec![StorageAction::Set(encoded_key, value.encode())]
                    }
                }
            }
            ReducerOutput::UtxosByScriptHash(utxos_by_script_hash::Output {
                script_hash,
                height,
                utxo_hash,
                utxo_index,
                action,
            }) => {
                let key = timbre_xbt::reducers::utxos_by_script_hash::Key {
                    script_hash,
                    height,
                    utxo_hash,
                    utxo_index,
                };

                let encoded_key = self.key_encoder.data(&Reducer::UtxosByScriptHash, &key);

                match action {
                    UtxoAction::Consumed => vec![StorageAction::Delete(encoded_key)],
                    UtxoAction::Produced(satoshis) => {
                        let value = timbre_xbt::reducers::utxos_by_script_hash::Value { satoshis };

                        vec![StorageAction::Set(encoded_key, value.encode())]
                    }
                }
            }
            ReducerOutput::Cursor(point, was_mempool, timestamp) => {
                let value = CursorValue {
                    height: point.height,
                    hash: point.hash.to_byte_array(),
                    was_mempool,
                    timestamp,
                    mempool_info: None,
                };

                vec![
                    StorageAction::Set(self.key_encoder.cursor(), value.encode()),
                    StorageAction::Set(self.key_encoder.info(), value.encode()),
                ]
            }
        }
    }

    /// Given a transaction and a StorageAction, perform the required operations
    /// against to database to execute the storage action.
    async fn execute_storage_op_in_txn(
        &self,
        txn: &mut Transaction,
        op: StorageAction,
        kv_map: &mut HashMap<Vec<u8>, Vec<u8>>,
    ) -> Result<(), gasket::error::Error> {
        match op {
            Set(k, v) => txn.put(k, v).await.or_restart(),
            Delete(k) => txn.delete(k).await.or_restart(),
            Insert(k, v) => match kv_map.remove(&k) {
                Some(_) => Ok(()),
                None => txn.put(k.clone(), v).await.or_restart(),
            },
            IncrementU64(k, d) => {
                if d == 0 {
                    return Ok(());
                };

                match kv_map.remove(&k) {
                    Some(prev_value) => {
                        let prev_amount = u64::from_be_bytes(prev_value[0..8].try_into().unwrap());

                        let new_value = prev_amount + d;

                        txn.put(k, u64::to_be_bytes(new_value)).await.or_restart()
                    }
                    None => txn.put(k, u64::to_be_bytes(d)).await.or_restart(),
                }
            }
            DecrementU64(k, d) => {
                if d == 0 {
                    return Ok(());
                };

                match kv_map.remove(&k) {
                    Some(prev_value) => {
                        let prev_amount = u64::from_be_bytes(prev_value[0..8].try_into().unwrap());

                        match prev_amount.checked_sub(d) {
                            Some(0) => txn.delete(k).await.or_restart(),
                            Some(v) => txn.put(k, u64::to_be_bytes(v)).await.or_restart(),
                            None => {
                                warn!(
                                    "Trying to decrement u64 key by more than it's current value: [{}] {prev_amount} - {d}, deleting key",
                                    hex::encode(k.clone())
                                );

                                txn.delete(k).await.or_restart()
                            }
                        }
                    }
                    None => Ok(tracing::error!(
                        "Trying to decrement u64 key which does not exist: [{}] - {d}, doing nothing",
                        hex::encode(k)
                    )),
                }
            }
            IncrementU128(k, d) => {
                if d == 0 {
                    return Ok(());
                };

                match kv_map.remove(&k) {
                    Some(prev_value) => {
                        let prev_amount =
                            u128::from_be_bytes(prev_value[0..16].try_into().unwrap());

                        let new_value = prev_amount + d;

                        txn.put(k, u128::to_be_bytes(new_value)).await.or_restart()
                    }
                    None => txn.put(k, u128::to_be_bytes(d)).await.or_restart(),
                }
            }
            DecrementU128(k, d) => {
                if d == 0 {
                    return Ok(());
                };

                match kv_map.remove(&k) {
                    Some(prev_value) => {
                        let prev_amount = u128::from_be_bytes(prev_value[0..16].try_into().unwrap());

                        match prev_amount.checked_sub(d) {
                            Some(0) => txn.delete(k).await.or_restart(),
                            Some(v) => txn.put(k, u128::to_be_bytes(v)).await.or_restart(),
                            None => {
                                warn!(
                                    "Trying to decrement u128 key by more than it's current value: [{}] {prev_amount} - {d}, deleting key",
                                    hex::encode(k.clone())
                                );

                                txn.delete(k).await.or_restart()
                            }
                        }
                    }
                    None => Ok(tracing::error!(
                        "Trying to decrement u128 key which does not exist: [{}] - {d}, doing nothing",
                        hex::encode(k)
                    )),
                }
            }
            DecrementU128NoDelete(k, d) => {
                if d == 0 {
                    return Ok(());
                };

                match kv_map.remove(&k) {
                    Some(prev_value) => {
                        let prev_amount = u128::from_be_bytes(prev_value[0..16].try_into().unwrap());

                        match prev_amount.checked_sub(d) {
                            Some(v) => txn.put(k, u128::to_be_bytes(v)).await.or_restart(),
                            None => {
                                warn!(
                                    "Trying to decrement u128 key by more than it's current value: [{}] {prev_amount} - {d}, setting to 0",
                                    hex::encode(k.clone())
                                );

                                txn.put(k, u128::to_be_bytes(0)).await.or_restart()
                            }
                        }
                    }
                    None => Ok(tracing::error!(
                        "Trying to decrement u128 key which does not exist: [{}] - {d}, doing nothing",
                        hex::encode(k)
                    )),
                }
            }
        }
    }

    /// Same as above but does extra work in order to return the StorageAction
    /// which will revert the effects of the action being applied.
    /// Derives the inverse of each storage action so a rollback can undo it.
    async fn execute_storage_op_in_txn_with_inverse(
        &self,
        txn: &mut Transaction,
        op: StorageAction,
        kv_map: &mut HashMap<Vec<u8>, Vec<u8>>,
    ) -> Result<Option<StorageAction>, gasket::error::Error> {
        match op {
            Set(k, v) => {
                // TODO key clones
                match kv_map.remove(&k) {
                    Some(prev_value) => {
                        if *prev_value == v {
                            // exact key value pair already exists, do nothing
                            Ok(None)
                        } else {
                            txn.put(k.clone(), v).await.or_restart()?;

                            Ok(Some(StorageAction::Set(k, prev_value)))
                        }
                    }
                    None => {
                        txn.put(k.clone(), v).await.or_restart()?;

                        Ok(Some(StorageAction::Delete(k)))
                    }
                }
            }
            Delete(k) => match kv_map.remove(&k) {
                Some(prev_value) => {
                    txn.delete(k.clone()).await.or_restart()?;

                    Ok(Some(StorageAction::Set(k, prev_value)))
                }
                None => {
                    // TODO: config (this false positives when we merge a Delete on a Set)
                    // tracing::warn!("Trying to delete a non-existent key: {}", hex::encode(k));

                    Ok(None)
                }
            },
            Insert(k, v) => match kv_map.remove(&k) {
                Some(_) => Ok(None),
                None => {
                    txn.put(k.clone(), v).await.or_restart()?;

                    Ok(Some(StorageAction::Delete(k)))
                }
            },
            IncrementU64(k, d) => match kv_map.remove(&k) {
                Some(prev_value) => {
                    let prev_amount = u64::from_be_bytes(prev_value[0..8].try_into().unwrap());

                    let new_value = prev_amount + d;

                    txn.put(k.clone(), u64::to_be_bytes(new_value))
                        .await
                        .or_restart()?;

                    Ok(Some(StorageAction::Set(k, prev_value)))
                }
                None => {
                    txn.put(k.clone(), u64::to_be_bytes(d)).await.or_restart()?;

                    Ok(Some(StorageAction::Delete(k)))
                }
            },
            DecrementU64(k, d) => match kv_map.remove(&k) {
                Some(prev_value) => {
                    let prev_amount = u64::from_be_bytes(prev_value[0..8].try_into().unwrap());

                    match prev_amount.checked_sub(d) {
                        Some(0) => {
                            txn.delete(k.clone()).await.or_restart()?;
                        }
                        Some(v) => {
                            txn.put(k.clone(), u64::to_be_bytes(v)).await.or_restart()?;
                        }
                        None => {
                            warn!(
                                "Trying to decrement u64 key by more than it's current value: [{}] {prev_amount} - {d}, deleting key",
                                hex::encode(k.clone())
                            );

                            txn.delete(k.clone()).await.or_restart()?;
                        }
                    }

                    Ok(Some(StorageAction::Set(k, prev_value)))
                }
                None => {
                    tracing::warn!("Trying to decrement u64 key which does not exist: [{}] - {d}, doing nothing", hex::encode(k));

                    Ok(None)
                }
            },
            IncrementU128(k, d) => match kv_map.remove(&k) {
                Some(prev_value) => {
                    let prev_amount = u128::from_be_bytes(prev_value[0..16].try_into().unwrap());

                    let new_value = prev_amount + d;

                    txn.put(k.clone(), u128::to_be_bytes(new_value))
                        .await
                        .or_restart()?;

                    Ok(Some(StorageAction::Set(k, prev_value)))
                }
                None => {
                    txn.put(k.clone(), u128::to_be_bytes(d))
                        .await
                        .or_restart()?;

                    Ok(Some(StorageAction::Delete(k)))
                }
            },
            DecrementU128(k, d) => match kv_map.remove(&k) {
                Some(prev_value) => {
                    let prev_amount = u128::from_be_bytes(prev_value[0..16].try_into().unwrap());

                    match prev_amount.checked_sub(d) {
                        Some(0) => {
                            txn.delete(k.clone()).await.or_restart()?;
                        }
                        Some(v) => {
                            txn.put(k.clone(), u128::to_be_bytes(v))
                                .await
                                .or_restart()?;
                        }
                        None => {
                            warn!(
                                "Trying to decrement u128 key by more than it's current value: [{}] {prev_amount} - {d}, deleting key",
                                hex::encode(k.clone())
                            );

                            txn.delete(k.clone()).await.or_restart()?;
                        }
                    }

                    Ok(Some(StorageAction::Set(k, prev_value)))
                }
                None => {
                    tracing::warn!("Trying to decrement u128 key which does not exist: [{}] - {d}, doing nothing", hex::encode(k));

                    Ok(None)
                }
            },
            DecrementU128NoDelete(k, d) => match kv_map.remove(&k) {
                Some(prev_value) => {
                    let prev_amount = u128::from_be_bytes(prev_value[0..16].try_into().unwrap());

                    match prev_amount.checked_sub(d) {
                        Some(v) => {
                            txn.put(k.clone(), u128::to_be_bytes(v))
                                .await
                                .or_restart()?;
                        }
                        None => {
                            warn!(
                                "Trying to decrement u128 key by more than it's current value: [{}] {prev_amount} - {d}, setting to 0",
                                hex::encode(k.clone())
                            );

                            txn.put(k.clone(), u128::to_be_bytes(0))
                                .await
                                .or_restart()?;
                        }
                    }

                    Ok(Some(StorageAction::Set(k, prev_value)))
                }
                None => {
                    tracing::warn!("Trying to decrement u64 key which does not exist: [{}] - {d}, doing nothing", hex::encode(k));

                    Ok(None)
                }
            },
        }
    }

    async fn apply_storage_rb_actions(
        &mut self,
        txn: &mut Transaction,
        inverse_ops: &StorageActions,
        b_height: u64,
        b_hash: &BlockHash,
    ) -> Result<(), gasket::error::Error> {
        for (idx, action) in inverse_ops.iter().enumerate() {
            let key = self.key_encoder.rollback(MetadataKey {
                height: b_height,
                hash: b_hash.to_byte_array(),
                operation_idx: idx as u64,
                modified_key: action.key().clone(),
            });

            // Value is NIL if the inverse action deleting the key, otherwise is
            // the overwritten value.
            let value = match action {
                Set(_, v) => v.clone(),
                Delete(_) => vec![],
                d => {
                    tracing::error!("inverse operation was not 'Set' or 'Delete': {d:?}");

                    return Err(gasket::error::Error::WorkPanic);
                }
            };

            debug!(
                "write storage rb [{}] -> [{}]",
                hex::encode(&key),
                hex::encode(&value)
            );

            txn.put(key, value).await.or_restart()?
        }

        Ok(())
    }

    async fn garbage_collect_rollback_metadata(
        &mut self,
        b_height: u64,
    ) -> Result<usize, gasket::error::Error> {
        // garbage collect (delete) old rollback metadata keys
        if b_height >= (self.buffer_size as u64) {
            let gc_range = rollback_metadata_key_range(
                self.key_encoder.namespace(),
                None::<u64>,
                Some(b_height - self.buffer_size as u64),
            );

            let gc_range = gc_range.0..gc_range.1;

            debug!(
                "garbage collecting rb range: [{}] - [{}]",
                hex::encode(&gc_range.start),
                hex::encode(&gc_range.end)
            );

            let mut total = 0;

            // start db transaction
            let mut txn = self.begin_tikv_transaction().await.or_restart()?;

            let mut current_height = None;

            loop {
                // scan gc keys
                let old_keys: Vec<Key> = txn
                    .scan_keys(gc_range.clone(), 1000)
                    .await
                    .or_restart()?
                    .collect();

                if old_keys.is_empty() {
                    break;
                }

                for k in old_keys {
                    let k_bytes = Into::<Vec<u8>>::into(k);

                    let k_bytes_trunc = k_bytes.get(4..).unwrap();
                    let (metadata_key, _) = MetadataKey::decode(&k_bytes_trunc).unwrap();
                    if let Some(ch) = current_height {
                        if ch != metadata_key.height {
                            commit_txn_or_rollback(&mut txn).await.or_restart()?;

                            debug!("gc'd rb height [{}]", ch);

                            // start db transaction
                            txn = self.begin_tikv_transaction().await.or_restart()?;

                            current_height = Some(metadata_key.height);
                        }
                    } else {
                        current_height = Some(metadata_key.height);
                    }

                    txn.delete(Key::from(k_bytes.to_vec())).await.or_restart()?;

                    total += 1;
                }
            }

            commit_txn_or_rollback(&mut txn).await.or_restart()?;

            debug!("gc'd rb height [{:?}] ({total})", current_height);

            Ok(total)
        } else {
            Ok(0)
        }
    }

    /// Given a list of storage actions, execute them against the database. If
    /// return_inverses flag is set the function returns the storage actions
    /// which will inverse the given actions (same ordering as they were applied).
    ///
    /// BATCH GET:
    /// - if the action is INCR/DECR AND/OR rollback handling is enabled, then
    /// we need to get a key for the action. if we batch get all the keys, and then
    /// pass these as a map to the execute storage op functions.
    async fn apply_actions(
        &mut self,
        txn: &mut Transaction,
        actions: Vec<StorageAction>,
        return_inverses: bool,
    ) -> Result<Option<Vec<StorageAction>>, gasket::error::Error> {
        // function that given a list of actions and whether or not we need inverses,
        // returns a map of all the required kvs from the db by batch getting the keys
        let mut kv_map = self
            .batch_get_required_kvs_for_batch(txn, &actions, return_inverses)
            .await?;

        debug!("batch fetched required {} kvs", kv_map.len());

        if return_inverses {
            let mut inverses: Vec<StorageAction> = Vec::new();

            for action in actions {
                debug!("executing storage op with inverse: {action:?}");

                if let Some(inverse_op) = self
                    .execute_storage_op_in_txn_with_inverse(txn, action, &mut kv_map)
                    .await
                    .or_restart()?
                {
                    debug!("storing inverse storage op: {inverse_op:?}");
                    inverses.push(inverse_op)
                }
            }

            Ok(Some(inverses))
        } else {
            for action in actions {
                debug!("executing storage op: {action:?}");

                self.execute_storage_op_in_txn(txn, action, &mut kv_map)
                    .await
                    .or_restart()?
            }

            Ok(None)
        }
    }

    /// Apply Sets, Deletes, and then others in separate db transactions
    async fn apply_staggered_actions(
        &mut self,
        actions: Vec<StorageAction>,
    ) -> Result<(), gasket::error::Error> {
        let mut dels = Vec::new();
        let mut sets = Vec::new();
        let mut others = Vec::new();
        let mut cursor_action = None;

        for action in actions {
            match action {
                Delete(_) => dels.push(action),
                Set(ref x, _) if *x == self.key_encoder.cursor() => cursor_action = Some(action),
                Set(_, _) => sets.push(action),
                _ => others.push(action),
            }
        }

        // deletes

        debug!("applying {} deletes", dels.len());

        // start db transaction
        let mut txn = self.begin_tikv_transaction().await.or_restart()?;

        for action in dels {
            debug!("executing storage op: {action:?}");

            self.execute_storage_op_in_txn(&mut txn, action, &mut HashMap::new())
                .await
                .or_restart()?
        }

        commit_txn_or_rollback(&mut txn).await.or_restart()?;

        // chunked sets (without cursor action)

        for set_batch in sets.into_iter().chunks(5000).into_iter() {
            let set_batch: Vec<_> = set_batch.collect();

            debug!("applying sets chunk with {} actions", set_batch.len());

            // start db transaction
            let mut txn = self.begin_tikv_transaction().await.or_restart()?;

            let mut kv_map = self
                .batch_get_required_kvs_for_batch(&mut txn, &set_batch, false)
                .await?;

            for action in set_batch {
                debug!("executing storage op: {action:?}");

                self.execute_storage_op_in_txn(&mut txn, action, &mut kv_map)
                    .await
                    .or_restart()?;
            }

            commit_txn_or_rollback(&mut txn).await.or_restart()?;
        }

        // others, with updated cursor set action

        if let Some(cursor) = cursor_action {
            others.push(cursor)
        } else {
            panic!("no cursor action found")
        }

        debug!("finally, applying {} others including cursor", others.len());

        // start db transaction
        let mut txn = self.begin_tikv_transaction().await.or_restart()?;

        let mut kv_map = self
            .batch_get_required_kvs_for_batch(&mut txn, &others, false)
            .await?;

        for action in others {
            debug!("executing storage op: {action:?}");

            self.execute_storage_op_in_txn(&mut txn, action, &mut kv_map)
                .await
                .or_restart()?
        }

        commit_txn_or_rollback(&mut txn).await.or_restart()?;

        Ok(())
    }

    async fn trim_persistent_rollback_metadata(
        &mut self,
        after_height: u64,
    ) -> Result<usize, gasket::error::Error> {
        let range = rollback_metadata_key_range(
            self.key_encoder.namespace(),
            Some(after_height + 1),
            None::<u64>,
        );

        let range = range.0..range.1;

        debug!(
            "trimming persistent buffer range: [{}] - [{}]",
            hex::encode(&range.start),
            hex::encode(&range.end)
        );

        let mut total = 0;

        // start db transaction
        let mut txn = self.begin_tikv_transaction().await.or_restart()?;

        let mut current_height = None;

        loop {
            // scan gc keys
            let old_keys: Vec<Key> = txn
                .scan_keys_reverse(range.clone(), 1000)
                .await
                .or_restart()?
                .collect();

            if old_keys.is_empty() {
                break;
            }

            for k in old_keys {
                let k_bytes = Into::<Vec<u8>>::into(k);

                let k_bytes_trunc = k_bytes.get(4..).unwrap();
                let (metadata_key, _) = MetadataKey::decode(&k_bytes_trunc).unwrap();
                if let Some(ch) = current_height {
                    if ch != metadata_key.height {
                        commit_txn_or_rollback(&mut txn).await.or_restart()?;

                        info!("removed persistent rb buffer height [{}]", ch);

                        // start db transaction
                        txn = self.begin_tikv_transaction().await.or_restart()?;

                        current_height = Some(metadata_key.height);
                    }
                } else {
                    current_height = Some(metadata_key.height);
                }

                txn.delete(Key::from(k_bytes.to_vec())).await.or_restart()?;

                total += 1;
            }
        }

        commit_txn_or_rollback(&mut txn).await.or_restart()?;

        debug!(
            "trimmed persistent rb buffer after height [{:?}] ({total})",
            after_height
        );

        Ok(total)
    }

    async fn batch_get_required_kvs_for_batch(
        &mut self,
        txn: &mut Transaction,
        actions: &StorageActions,
        need_inverses: bool,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, gasket::error::Error> {
        let required_keys: Vec<Vec<u8>> = if need_inverses {
            // if we need inverses, then we require all keys
            actions.iter().map(|a| a.key().clone()).collect()
        } else {
            // if not, we only need the keys for incr/decrs/inserts
            actions
                .iter()
                .filter(|a| a.requires_previous_value())
                .map(|a| a.key().clone())
                .collect()
        };

        // split the required keys into smaller batches to avoid large response size
        // which can cause an error

        let mut acc = HashMap::new();

        for key_batch in required_keys.chunks(1000) {
            let kvs: HashMap<Vec<u8>, Vec<u8>> = txn
                .batch_get(key_batch.to_vec())
                .await
                .or_restart()?
                .map(|KvPair(k, v)| (k.into(), v))
                .collect();

            acc.extend(kvs)
        }

        Ok(acc)
    }
}

#[async_trait::async_trait(?Send)]
impl gasket::runtime::Worker for Worker {
    type WorkUnit = StorageActionPayload;

    fn metrics(&self) -> gasket::metrics::Registry {
        gasket::metrics::Builder::new()
            .with_counter("storage_ops", &self.ops_count)
            .build()
    }

    async fn bootstrap(&mut self) -> Result<(), gasket::error::Error> {
        info!(
            "bootstrapping storage (dataplane {}, instance {}, network {})",
            self.config.dataplane_id, self.config.instance_id, self.config.network
        );

        self.connection = TransactionClient::new_with_config(
            vec![self.config.connection_params.clone()],
            tikv_client::Config::default(),
        )
        .await
        .or_restart()?
        .into();

        self.redis_connection = redis::Client::open(self.config.redis_address.clone())
            .or_restart()?
            .into();

        let current_ts = self
            .connection
            .as_mut()
            .unwrap()
            .current_timestamp()
            .await
            .or_restart()?;

        if self.config.tikv_cleanup_locks.unwrap_or(true) {
            // clean up any lingering locks on any key for this instance
            self.cleanup_tikv_locks(&current_ts, false)
                .await
                .or_restart()?;
        }

        /*
            There can be a situation where polyphony ends after committing the database transaction
            processing a block(s) BUT BEFORE writing the corresponding entry to the Redis. Here we
            try to detect and resolve that by creating the Redis entry now with the current ts.
        */

        let mut snapshot = self.connection.as_mut().unwrap().snapshot(
            current_ts.clone(),
            TransactionOptions::new_optimistic().drop_check(tikv_client::CheckLevel::Warn),
        );

        let raw_cursor = snapshot.get(self.key_encoder.cursor()).await.or_restart()?;

        if let Some(cursor_bytes) = raw_cursor {
            let ((height, hash), rem_bytes) = <_>::decode(&cursor_bytes).unwrap();

            let ts_key = self.timestamps_key();
            let redis_entries: Vec<RedisEntry> = self
                .redis_connection
                .as_mut()
                .unwrap()
                .zrangebyscore(ts_key.clone(), "-inf", "+inf")
                .or_restart()?;

            let have_redis_entries = !redis_entries.is_empty();

            let redis_entries_contains_cursor = redis_entries
                .iter()
                .position(|r| r.height == height && r.block_hash == hash)
                .is_some();

            // if we have redis entries, but we can't find a redis entry for the current cursor in tikv,
            // then insert a new entry with the current timestamp
            if have_redis_entries && !redis_entries_contains_cursor {
                warn!(
                    "no redis entry found for current tikv cursor: {:?}",
                    (height, hash)
                );

                if !rem_bytes.is_empty() {
                    info!("inserting a new redis entry for the tikv cursor with ts {current_ts:?}");

                    let (was_mempool, _) = bool::decode(&rem_bytes).unwrap();

                    let ts_key = self.timestamps_key();
                    let _: u32 = self
                        .redis_connection
                        .as_mut()
                        .unwrap()
                        .zadd(
                            ts_key.clone(),
                            RedisEntry {
                                height,
                                block_hash: hash,
                                was_mempool,
                                commit_ts: current_ts.clone(),
                                network: self.config.network.clone(),
                            },
                            current_ts.physical,
                        )
                        .or_restart()?;

                    self.index_timestamps_key(current_ts.physical)?;
                } else {
                    warn!("old cursor version, not adding an entry to redis")
                }
            }
        }

        Ok(())
    }

    async fn schedule(&mut self) -> ScheduleResult<Self::WorkUnit> {
        // if we haven't committed the last work unit then reschedule that, else
        // pull the next message from the upstream stage
        match &self.work_unit_2pc {
            Some(w) => {
                info!("found uncommitted 2pc work unit, re-scheduling...");
                Ok(WorkSchedule::Unit(w.clone()))
            }
            None => {
                debug!("scheduling");
                // receive the MsgRollForward/Backwards (and list of storage
                // actions so we can add them to the persistent rollback buffer)
                let msg = self.input.recv().await?;

                Ok(WorkSchedule::Unit(msg.payload))
            }
        }
    }

    async fn execute(&mut self, unit: &Self::WorkUnit) -> Result<(), gasket::error::Error> {
        match unit {
            StorageActionPayload::RollForward(point, outputs, mutable, mempool) => {
                tracing::debug!(
                    "processing roll forwards msg for point {point:?} (batcher actions: {}) (mutable: {mutable}) (mempool: {mempool})",
                    self.sync_batcher.batch_actions.len()
                );

                self.mutable |= mutable;

                // convert the reducer outputs into a list of storage actions
                // to be executed against the db
                let reducer_storage_ops: StorageActions = outputs
                    .clone()
                    .into_iter()
                    .map(|x| self.reducer_output_to_storage_ops(x))
                    .flatten()
                    .collect();

                // If chain mutable:
                // - if it is the first block we are processing as mutable, we
                // need to add the last point to the rollback buffer in case
                // we see a rollback to that point
                // - check if sync batcher contains any actions, if so flush/apply them
                // without returning inverses
                // - apply block actions and get inverse actions
                // - apply inverse actions stuff and gc

                if !self.mutable {
                    // chain not mutable: no rollback handling, batched txs

                    if self.sync_batcher.should_flush(&reducer_storage_ops) {
                        // 2pc lock
                        self.work_unit_2pc = Some(unit.clone());

                        let actions = self.sync_batcher.get_batch_actions();

                        if actions.len() > 5000 {
                            warn!("flushing large batch with {} actions", actions.len());

                            self.apply_staggered_actions(actions).await?;
                        } else {
                            debug!("flushing batch with {} actions", actions.len());

                            // start db transaction
                            let mut txn = self.begin_tikv_transaction().await.or_restart()?;

                            let _ = self.apply_actions(&mut txn, actions, false).await?;

                            commit_txn_or_rollback(&mut txn).await.or_restart()?;
                        }

                        // 2pc unlock
                        self.work_unit_2pc = None;

                        self.last_processed = Some(point.clone());

                        // no-op unless advertise_immediately is set: bulk-mode
                        // instances stay invisible until the mutable window
                        self.maybe_advertise(point.height)?;

                        self.ops_count
                            .inc(self.sync_batcher.batch_actions.len() as u64);

                        self.sync_batcher.wipe();

                        debug!(
                            "initialising new batch with {} actions",
                            reducer_storage_ops.len()
                        );

                        self.sync_batcher.push_actions(reducer_storage_ops);

                        Ok(())
                    } else {
                        self.sync_batcher.push_actions(reducer_storage_ops);

                        Ok(())
                    }
                } else {
                    let first_mutable = self.rollback_buffer.is_empty();

                    // 2pc lock
                    self.work_unit_2pc = Some(unit.clone());

                    let mut total_ops = 0;

                    // check if any actions remain in batch from sync, if so apply/flush them
                    if !self.sync_batcher.is_empty() {
                        let rem_actions = self.sync_batcher.get_batch_actions();

                        debug!(
                            "applying {} actions remaining in sync batcher...",
                            rem_actions.len()
                        );

                        self.apply_staggered_actions(rem_actions).await?;

                        self.sync_batcher.wipe();
                    }

                    // chain is mutable: rollback handling, no batch txs
                    debug!("starting tx");

                    // start db transaction
                    let mut txn = self.begin_tikv_transaction().await.or_restart()?;

                    // create a syncbatcher so we can use its merging
                    // functionality for this single block
                    let mut merger = SyncBatcher::new(1, usize::MAX);
                    merger.push_actions(reducer_storage_ops);

                    let actions = merger.get_batch_actions();

                    debug!("applying {} actions", actions.len());

                    let inverse_ops = self.apply_actions(&mut txn, actions, true).await?.unwrap();

                    // initialise the persistent rollback buffer if it is empty
                    // and this is the first mutable block (we do the same for
                    // in-memory buffer after db tx success)
                    if first_mutable {
                        if let Some(p) = self.last_processed {
                            self.apply_storage_rb_actions(
                                &mut txn,
                                &vec![StorageAction::Delete(vec![])],
                                p.height,
                                &p.hash,
                            )
                            .await?
                        }
                    }

                    debug!("applying rb actions ({} storage)", inverse_ops.len());

                    self.apply_storage_rb_actions(
                        &mut txn,
                        &inverse_ops,
                        point.height,
                        &point.hash,
                    )
                    .await?;

                    total_ops += inverse_ops.len();

                    debug!("committing tx");

                    let commit_ts = commit_txn_or_rollback(&mut txn).await.or_restart()?;

                    // now we have successfully committed the block we can remove the 2pc unit
                    self.work_unit_2pc = None;

                    // initialise the in-memory rollback buffer by pushing the
                    // last point, in case we need to rollback to it
                    if first_mutable {
                        let init_point = self.last_processed.clone().unwrap_or(Point {
                            height: 0,
                            hash: BlockHash::from_byte_array([0; 32]),
                        });
                        info!("initialising rollback buffer with {:?}", init_point);
                        self.rollback_buffer.add_block(init_point, vec![])
                    }

                    self.last_processed = Some(point.clone());

                    // push the point and the inverse storage actions onto the
                    // front of the in-memory rollback buffer
                    self.rollback_buffer.add_block(point.clone(), inverse_ops);

                    self.ops_count.inc(total_ops as u64);

                    // store the commit timestamp in redis
                    if let Some(ts) = commit_ts {
                        if self.config.tikv_cleanup_locks.unwrap_or(true) {
                            self.cleanup_tikv_locks(&ts, true).await.or_restart()?;
                        }

                        // add the timestamp entry for the committed block
                        let ts_key = self.timestamps_key();
                        let _: redis::Value = self
                            .redis_connection
                            .as_mut()
                            .unwrap()
                            .get_connection()
                            .or_restart()?
                            .zadd(
                                ts_key,
                                RedisEntry {
                                    height: point.height,
                                    block_hash: *point.hash.as_byte_array(),
                                    was_mempool: *mempool,
                                    commit_ts: ts.clone(),
                                    network: self.config.network.clone(),
                                },
                                ts.physical,
                            )
                            .or_restart()?;

                        self.index_timestamps_key(ts.physical)?;
                        self.maybe_advertise(point.height)?;
                    }

                    debug!("gcing rb actions");

                    self.garbage_collect_rollback_metadata(point.height).await?;

                    Ok(())
                }
            }
            // TODO mempool flag?
            StorageActionPayload::RollBack(point, mutable) => {
                self.mutable |= mutable;

                // 2pc lock
                self.work_unit_2pc = Some(unit.clone());

                tracing::info!("processing roll backwards msg for point {point:?}");

                // fetch the required points and inverse storage actions from
                // the memory rollback buffer
                let points_and_results = self
                    .rollback_buffer
                    .points_since(&point)
                    .map_err(crate::Error::rollback)
                    .apply_policy(&self.policy)
                    .or_panic()?;

                // apply the error policy for unhandleable rollbacks
                let points_and_results = match points_and_results {
                    Some(x) => x,
                    None => panic!("could not handle rollback"),
                };

                debug!(
                    "found {} points in rb buf after rb point",
                    points_and_results.len()
                );

                let mut rollback_final_ts = None;

                // for each rollbacked block, starting with the most recent
                for p in points_and_results.into_iter() {
                    // apply inverse actions in opposite order to original actions
                    let inverse_actions = p.result.into_iter().rev().collect();

                    let mut txn = self.begin_tikv_transaction().await.or_restart()?;

                    // apply all the inverse actions
                    self.apply_actions(&mut txn, inverse_actions, false).await?;

                    debug!("committing batch");

                    rollback_final_ts = commit_txn_or_rollback(&mut txn).await.or_restart()?;
                }

                // add a fresh timestamp entry for the rollback point
                if let Some(ts) = rollback_final_ts {
                    if self.config.tikv_cleanup_locks.unwrap_or(true) {
                        self.cleanup_tikv_locks(&ts, true).await.or_restart()?;
                    }

                    let ts_key = self.timestamps_key();
                    let _: redis::Value = self
                        .redis_connection
                        .as_mut()
                        .unwrap()
                        .get_connection()
                        .or_restart()?
                        .zadd(
                            ts_key,
                            RedisEntry {
                                height: point.height,
                                block_hash: *point.hash.as_byte_array(),
                                was_mempool: false,
                                commit_ts: ts.clone(),
                                network: self.config.network.clone(),
                            },
                            ts.physical,
                        )
                        .or_restart()?;

                    self.index_timestamps_key(ts.physical)?;
                    self.maybe_advertise(point.height)?;
                }

                debug!("trimming persistent");

                // remove the rollbacked points from the persistent rollback buffer
                self.trim_persistent_rollback_metadata(point.height).await?;

                // 2pc unlock
                self.work_unit_2pc = None;

                self.last_processed = Some(point.clone());

                // Now we have successfully sent a transaction to storage using data from the memory
                // rollback buffer, we can trim those points from the buffer.
                self.rollback_buffer
                    .rollback_to_point(&point)
                    .map_err(crate::Error::rollback)
                    .apply_policy(&self.policy)
                    .or_panic()?;

                // count the number of storage actions which were performed

                Ok(())
            }
        }
    }

    async fn teardown(&mut self) -> Result<(), gasket::error::Error> {
        Ok(())
    }
}

async fn commit_txn_or_rollback(
    txn: &mut Transaction,
) -> Result<Option<Timestamp>, tikv_client::Error> {
    match txn.commit().await {
        Ok(ts) => {
            debug!(
                "finished committing: {ts:?} ({:?})",
                ts.as_ref().map(|t| t.version())
            );

            Ok(ts)
        }
        e @ Err(_) => {
            info!("error while committing, rolling back txn...");

            txn.rollback().await?;

            info!("rollbacked database txn successfully");

            e
        }
    }
}
