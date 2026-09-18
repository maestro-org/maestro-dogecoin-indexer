use std::ops::Range;

use bb8::Pool;
use bb8_tikv::TiKVTransactionalConnectionManager;
use bitcoin::{hashes::Hash, BlockHash};
use tikv_client::{Key, KvPair, TransactionOptions};
use timbre_xbt::{
    reducers::{
        inscriptions_by_utxo, rune_id_by_rune_name, runes_by_utxo, script_by_script_hash,
        utxos_by_script_hash::{Key as UtxosByScriptHashKey, Value as UtxosByScriptHashValue},
    },
    Decode, Encode, Prefix, Reducer,
};
use tracing::warn;

use crate::{
    error::{Error, MapiResult},
    key_resolver::{self, ReducerType},
    options::Mode,
    types::{LastUpdated, OrderParam},
    util::DogecoinAddress,
};

// when scanning many keys, scan in batches of this size
static KV_SCAN_BATCH_SIZE: u32 = 1000;

#[derive(Clone)]
pub struct PolyphonyWrapper {
    pub pool: Pool<TiKVTransactionalConnectionManager>,
}

impl PolyphonyWrapper {
    pub fn new(pool: Pool<TiKVTransactionalConnectionManager>) -> Self {
        PolyphonyWrapper { pool }
    }

    pub async fn get_tikv_client(
        &self,
    ) -> Result<bb8::PooledConnection<'_, TiKVTransactionalConnectionManager>, Error> {
        self.pool
            .get()
            .await
            .map_err(|_| Error::Internal("Unable to get TiKV pool connection".into()))
    }

    pub async fn begin_snapshot_latest(&self) -> Result<Snapshot, Error> {
        let client = self.get_tikv_client().await?;

        let ts = client
            .current_timestamp()
            .await
            .map_err(|_| Error::Internal("tikv ts fetch".into()))?;

        let snap = client.snapshot(ts, TransactionOptions::new_optimistic());

        Ok(Snapshot(snap))
    }
    pub fn utxos_by_rune_id_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) = key_resolver::resolve_key(ReducerType::UtxosByRuneId)
            .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn etching_by_rune_id_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) = key_resolver::resolve_key(ReducerType::EtchingByRuneId)
            .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn balances_by_rune_id_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) =
            key_resolver::resolve_key(ReducerType::BalancesByRuneId)
                .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn mints_by_rune_id_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) = key_resolver::resolve_key(ReducerType::MintsByRuneId)
            .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn script_by_script_hash_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) = key_resolver::resolve_key(ReducerType::ScriptByScriptHash)
            .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn runes_by_utxo_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) = key_resolver::resolve_key(ReducerType::RunesByUtxo)
            .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn rune_id_by_name_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) =
            key_resolver::resolve_key(ReducerType::RuneIdByRuneName)
                .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn utxos_by_script_hash_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) = key_resolver::resolve_key(ReducerType::UtxosByScriptHash)
            .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn script_hash_by_address_payload_hash_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) =
            key_resolver::resolve_key(ReducerType::ScriptHashByAddressPayloadHash)
                .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn inscription_utxos_by_script_hash_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) =
            key_resolver::resolve_key(ReducerType::InscriptionUtxosByScriptHash)
                .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn inscriptions_by_utxo_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) = key_resolver::resolve_key(ReducerType::InscriptionsByUtxo)
            .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn content_by_inscription_id_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) =
            key_resolver::resolve_key(ReducerType::ContentByInscriptionId)
                .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn brc20_balances_by_script_hash_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) =
            key_resolver::resolve_key(ReducerType::Brc20BalancesByScriptHash)
                .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn brc20_terms_by_ticker_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) = key_resolver::resolve_key(ReducerType::Brc20TermsByTicker)
            .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn txs_by_script_hash_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) = key_resolver::resolve_key(ReducerType::TxsByScriptHash)
            .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn brc20_balances_by_ticker_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) =
            key_resolver::resolve_key(ReducerType::Brc20BalancesByTicker)
                .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn rune_balances_by_script_hash_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) =
            key_resolver::resolve_key(ReducerType::RuneBalancesByScriptHash)
                .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn transfer_inscriptions_by_script_hash_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) =
            key_resolver::resolve_key(ReducerType::TransferInscriptionsByAddress)
                .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub fn sat_balance_by_script_hash_encoder(&self) -> MapiResult<Prefix> {
        let (dataplane_id, reducer_id) =
            key_resolver::resolve_key(ReducerType::SatBalanceByScriptHash)
                .map_err(|e| Error::Internal(format!("instance resolution: {e}")))?;
        Ok(Prefix::new(dataplane_id, reducer_id))
    }

    pub async fn resolve_script_hash(
        &self,
        snapshot: &mut Snapshot,
        network: Mode,
        script_hash: [u8; 20],
        script_by_script_hash_encoder: &Prefix,
    ) -> MapiResult<(Option<DogecoinAddress>, Vec<u8>)> {
        let script_bytes: Vec<u8> = snapshot
            .get_reducer_key(
                &script_by_script_hash_encoder,
                &Reducer::ScriptByScriptHash,
                &script_by_script_hash::Key { script_hash },
            )
            .await?;

        let (addr, script) = match network {
            Mode::Dogecoin => {
                let script = dogecoin::Script::from(script_bytes);

                (
                    dogecoin::Address::from_script(&script, dogecoin::Network::Bitcoin)
                        .map(DogecoinAddress::Dogecoin),
                    script.into_bytes(),
                )
            }
            Mode::DogecoinTestnet => {
                let script = dogecoin::Script::from(script_bytes);

                (
                    dogecoin::Address::from_script(&script, dogecoin::Network::Testnet)
                        .map(DogecoinAddress::Dogecoin),
                    script.into_bytes(),
                )
            }
            Mode::GenerateOpenApi => {
                unreachable!("server never runs in GenerateOpenApi mode")
            }
        };

        Ok((addr, script))
    }

    pub async fn resolve_rune_name(
        &self,
        snapshot: &mut Snapshot,
        rune_name: u128,
    ) -> MapiResult<Option<(u64, u32)>> {
        Ok(snapshot
            .get_reducer_key_maybe::<_, rune_id_by_rune_name::Value>(
                &self.rune_id_by_name_encoder()?,
                &Reducer::RuneIdByRuneName,
                &rune_id_by_rune_name::Key { rune_name },
            )
            .await?
            .map(|x| x.rune_id))
    }
}

pub struct Snapshot(tikv_client::Snapshot);

impl Snapshot {
    pub async fn get_last_updated(&mut self, encoder: &Prefix) -> MapiResult<LastUpdated> {
        let cursor_bytes = self
            .0
            .get(encoder.cursor())
            .await
            .map_err(Error::TiKV)?
            .ok_or_else(|| Error::MissingData(encoder.cursor()))?;

        let (block_height, block_hash): (u64, [u8; 32]) = <_>::decode(&cursor_bytes)
            .map_err(|e| Error::MalformedData(cursor_bytes.clone(), Some(e)))?
            .0;

        Ok(LastUpdated {
            block_height,
            block_hash: BlockHash::from_byte_array(block_hash).to_string(),
        })
    }

    pub async fn get_reducer_key<A: Encode + Decode, B: Decode>(
        &mut self,
        encoder: &Prefix,
        reducer: &Reducer,
        key: &A,
    ) -> MapiResult<B> {
        self.get_reducer_key_maybe(encoder, reducer, key)
            .await?
            .ok_or_else(|| Error::MissingData(encoder.data(reducer, key)))
    }

    pub async fn get_reducer_key_maybe<A: Encode + Decode, B: Decode>(
        &mut self,
        encoder: &Prefix,
        reducer: &Reducer,
        key: &A,
    ) -> MapiResult<Option<B>> {
        let encoded_key = encoder.data(reducer, key);

        let encoded_value = self.0.get(encoded_key).await.map_err(Error::TiKV)?;

        match encoded_value {
            Some(v) => Ok(Some(
                <B>::decode(&v)
                    .map_err(|e| Error::MalformedData(v.clone(), Some(e)))?
                    .0,
            )),
            None => Ok(None),
        }
    }
}

#[derive(Clone)]
pub struct PolyphonyEncoders {
    pub utxos_by_rune_id: Prefix,
    pub etching_by_rune_id: Prefix,
    pub mints_by_rune_id: Prefix,
    pub script_by_script_hash: Prefix,
    pub runes_by_utxo: Prefix,
    pub rune_id_by_name: Prefix,
    pub utxos_by_script_hash: Prefix,
    pub script_hash_by_address_payload_hash: Prefix,
    pub inscription_utxos_by_script_hash: Prefix,
    pub inscriptions_by_utxo: Prefix,
    pub content_by_inscription_id: Prefix,
    pub brc20_balances_by_script_hash: Prefix,
    pub brc20_terms_by_ticker: Prefix,
    pub txs_by_script_hash: Prefix,
    pub brc20_balances_by_ticker: Prefix,
    pub rune_balances_by_script_hash: Prefix,
    pub transfer_inscriptions_by_script_hash: Prefix,
}

pub struct Scanner {
    pub range: Range<Vec<u8>>,
    pub order: OrderParam,
    pub count: Option<usize>,
}

impl Scanner {
    pub fn new(range: Range<Vec<u8>>) -> Self {
        Self {
            range,
            order: OrderParam::Asc,
            count: None,
        }
    }

    pub fn order(mut self, order: OrderParam) -> Scanner {
        self.order = order;

        self
    }

    pub fn count(mut self, count: usize) -> Scanner {
        self.count = Some(count);

        self
    }

    pub async fn execute<K, V>(self, snapshot: &mut Snapshot) -> MapiResult<Vec<(K, V)>>
    where
        K: Decode + Send + Sync,
        V: Decode + Send + Sync,
    {
        self.execute_with_filter::<K, V, fn(&KvPair) -> bool>(snapshot, None)
            .await
    }

    pub async fn execute_keys_only<K>(self, snapshot: &mut Snapshot) -> MapiResult<Vec<K>>
    where
        K: Decode + Send + Sync,
    {
        let mut scan_size = self.count.unwrap_or(1000) as u32;

        let mut range_remaining = self.range;
        let mut ks = Vec::new();

        'scan_loop: loop {
            let fetched = match self.order {
                OrderParam::Asc => snapshot
                    .0
                    .scan_keys(range_remaining.clone(), scan_size)
                    .await
                    .map(|iter| iter.collect::<Vec<Key>>()),
                OrderParam::Desc => snapshot
                    .0
                    .scan_keys_reverse(range_remaining.clone(), scan_size)
                    .await
                    .map(|iter| iter.collect::<Vec<Key>>()),
            };

            match fetched {
                Ok(ks_vec) => {
                    let num_kvs_fetched = ks_vec.len();

                    // use the last key fetched to update the remaining range, or
                    // break if no keys were fetched
                    range_remaining = if let Some(k) = ks_vec.last() {
                        let mut last_key = Into::<Vec<u8>>::into(k.clone());

                        // modify the range according to the order
                        match self.order {
                            OrderParam::Asc => {
                                last_key.push(0);
                                last_key..range_remaining.end
                            }
                            OrderParam::Desc => range_remaining.start..last_key,
                        }
                    } else {
                        break;
                    };

                    for key in ks_vec.into_iter() {
                        let k = Into::<Vec<u8>>::into(key);

                        // name space (2) || data || reducer || break
                        let k = k
                            .get(5..)
                            .ok_or_else(|| Error::MalformedData(k.clone(), None))?;

                        let key = K::decode(k)
                            .map_err(|e| Error::MalformedData(k.to_vec(), Some(e)))?
                            .0;

                        ks.push(key);

                        // if max_count was provided and we have enough kvs to satisfy
                        // that count then stop scanning
                        if let Some(max) = self.count {
                            if ks.len() >= max {
                                break 'scan_loop;
                            }
                        }
                    }

                    // if the number of total fetched (unfiltered) kvs is less than
                    // the scan size then we have exhausted the scan range, so stop
                    if (num_kvs_fetched as u32) < scan_size {
                        break;
                    }
                }
                Err(tikv_client::Error::Grpc(e))
                    if e.to_string().contains("Received message larger than max") =>
                {
                    warn!("scan size {scan_size} too large when fetching entire range {range_remaining:?}");
                    scan_size = (scan_size / 2).max(1)
                }
                Err(e) => return Err(Error::TiKV(e)),
            }
        }

        if let Some(max) = self.count {
            ks.truncate(max)
        }

        Ok(ks)
    }

    pub async fn execute_with_filter<K, V, F>(
        self,
        snapshot: &mut Snapshot,
        filter_fn: Option<F>,
    ) -> MapiResult<Vec<(K, V)>>
    where
        K: Decode + Send + Sync,
        V: Decode + Send + Sync,
        F: Fn(&KvPair) -> bool,
    {
        // if there is no filter we can just scan exactly the number of keys we want
        let mut scan_size = if filter_fn.is_none() {
            self.count.map(|x| x as u32).unwrap_or(KV_SCAN_BATCH_SIZE)
        } else {
            KV_SCAN_BATCH_SIZE
        };

        let mut range_remaining = self.range;
        let mut kvs = Vec::new();

        'scan_loop: loop {
            let fetched = match self.order {
                OrderParam::Asc => snapshot
                    .0
                    .scan(range_remaining.clone(), scan_size)
                    .await
                    .map(|iter| iter.collect::<Vec<KvPair>>()),
                OrderParam::Desc => snapshot
                    .0
                    .scan_reverse(range_remaining.clone(), scan_size)
                    .await
                    .map(|iter| iter.collect::<Vec<KvPair>>()),
            };

            match fetched {
                Ok(mut kvs_vec) => {
                    let num_kvs_fetched = kvs_vec.len();

                    // use the last key fetched to update the remaining range, or
                    // break if no keys were fetched
                    range_remaining = if let Some(kv) = kvs_vec.last() {
                        let mut last_key = Into::<Vec<u8>>::into(kv.clone().into_key());

                        // modify the range according to the order
                        match self.order {
                            OrderParam::Asc => {
                                last_key.push(0);
                                last_key..range_remaining.end
                            }
                            OrderParam::Desc => range_remaining.start..last_key,
                        }
                    } else {
                        break;
                    };

                    if let Some(f) = &filter_fn {
                        kvs_vec.retain(|kv| f(kv))
                    }

                    for KvPair(k, v) in kvs_vec {
                        let k = Into::<Vec<u8>>::into(k);

                        // name space (2) || data || reducer || break
                        let k = k
                            .get(5..)
                            .ok_or_else(|| Error::MalformedData(k.clone(), None))?;

                        let key = K::decode(k)
                            .map_err(|e| Error::MalformedData(k.to_vec(), Some(e)))?
                            .0;

                        let value = V::decode(&v)
                            .map_err(|e| Error::MalformedData(v.clone(), Some(e)))?
                            .0;

                        kvs.push((key, value));

                        // if max_count was provided and we have enough kvs to satisfy
                        // that count then stop scanning
                        if let Some(max) = self.count {
                            if kvs.len() >= max {
                                break 'scan_loop;
                            }
                        }
                    }

                    // if the number of total fetched (unfiltered) kvs is less than
                    // the scan size then we have exhausted the scan range, so stop
                    if (num_kvs_fetched as u32) < scan_size {
                        break;
                    }
                }
                Err(tikv_client::Error::Grpc(e))
                    if e.to_string().contains("Received message larger than max") =>
                {
                    warn!("scan size {scan_size} too large when fetching entire range {range_remaining:?}");
                    scan_size = (scan_size / 2).max(1)
                }
                Err(e) => return Err(Error::TiKV(e)),
            }
        }

        if let Some(max) = self.count {
            kvs.truncate(max)
        }

        Ok(kvs)
    }

    // Similar to execute_with_filter, except that metaprotocol UTxOs are excluded
    pub async fn get_non_metaprotocol_utxos<F>(
        self,
        snapshot: &mut Snapshot,
        filter_fn: Option<F>,
        runes_by_utxo_encoder: &Prefix,
        inscriptions_by_utxo_encoder: &Prefix,
    ) -> MapiResult<Vec<(UtxosByScriptHashKey, UtxosByScriptHashValue)>>
    where
        F: Fn(&KvPair) -> bool,
    {
        let mut scan_size = KV_SCAN_BATCH_SIZE;

        let mut range_remaining = self.range;
        let mut kvs = Vec::new();

        'scan_loop: loop {
            let fetched = match self.order {
                OrderParam::Asc => snapshot
                    .0
                    .scan(range_remaining.clone(), scan_size)
                    .await
                    .map(|iter| iter.collect::<Vec<KvPair>>()),
                OrderParam::Desc => snapshot
                    .0
                    .scan_reverse(range_remaining.clone(), scan_size)
                    .await
                    .map(|iter| iter.collect::<Vec<KvPair>>()),
            };

            match fetched {
                Ok(mut kvs_vec) => {
                    let num_kvs_fetched = kvs_vec.len();

                    // use the last key fetched to update the remaining range, or
                    // break if no keys were fetched
                    range_remaining = if let Some(kv) = kvs_vec.last() {
                        let mut last_key = Into::<Vec<u8>>::into(kv.clone().into_key());

                        // modify the range according to the order
                        match self.order {
                            OrderParam::Asc => {
                                last_key.push(0);
                                last_key..range_remaining.end
                            }
                            OrderParam::Desc => range_remaining.start..last_key,
                        }
                    } else {
                        break;
                    };

                    if let Some(f) = &filter_fn {
                        kvs_vec.retain(|kv| f(kv))
                    }

                    for KvPair(k, v) in kvs_vec {
                        let k = Into::<Vec<u8>>::into(k);

                        // name space (2) || data || reducer || break
                        let k = k
                            .get(5..)
                            .ok_or_else(|| Error::MalformedData(k.clone(), None))?;

                        let key = UtxosByScriptHashKey::decode(k)
                            .map_err(|e| Error::MalformedData(k.to_vec(), Some(e)))?
                            .0;

                        let value = UtxosByScriptHashValue::decode(&v)
                            .map_err(|e| Error::MalformedData(v.clone(), Some(e)))?
                            .0;

                        // Evaluate whether this is a metaprotocol UTxO
                        // First, check whether it's related to the Dunes metaprotocol
                        let dunes: Option<runes_by_utxo::Value> = snapshot
                            .get_reducer_key_maybe::<runes_by_utxo::Key, runes_by_utxo::Value>(
                                runes_by_utxo_encoder,
                                &Reducer::RunesByUtxo,
                                &runes_by_utxo::Key {
                                    utxo_hash: key.utxo_hash,
                                    utxo_index: key.utxo_index,
                                },
                            )
                            .await?;
                        // Then, check whether it's related to the inscriptions metaprotocol
                        let inscriptions: Option<inscriptions_by_utxo::Value> = snapshot
                            .get_reducer_key_maybe::<inscriptions_by_utxo::Key, inscriptions_by_utxo::Value>(
                                inscriptions_by_utxo_encoder,
                                &Reducer::InscriptionsByUtxo,
                                &inscriptions_by_utxo::Key {
                                    utxo_hash: key.utxo_hash,
                                    utxo_index: key.utxo_index,
                                },
                            )
                            .await?;

                        // Filter metaprotocol UTxOs
                        if dunes.is_none() && inscriptions.is_none() {
                            kvs.push((key, value));

                            // if max_count was provided and we have enough kvs to satisfy
                            // that count then stop scanning
                            if let Some(max) = self.count {
                                if kvs.len() >= max {
                                    break 'scan_loop;
                                }
                            }
                        }
                    }

                    // if the number of total fetched (unfiltered) kvs is less than
                    // the scan size then we have exhausted the scan range, so stop
                    if (num_kvs_fetched as u32) < scan_size {
                        break 'scan_loop;
                    }
                }
                Err(tikv_client::Error::Grpc(e))
                    if e.to_string().contains("Received message larger than max") =>
                {
                    warn!("scan size {scan_size} too large when fetching entire range {range_remaining:?}");
                    scan_size = (scan_size / 2).max(1)
                }
                Err(e) => return Err(Error::TiKV(e)),
            }
        }

        if let Some(max) = self.count {
            kvs.truncate(max)
        }

        Ok(kvs)
    }
}
