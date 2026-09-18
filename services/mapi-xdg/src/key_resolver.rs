//! Resolves which indexer instance's data to read for each reducer.
//!
//! Every polyphony instance that has caught up to the chain tip registers
//! itself in a Redis sorted set per instance group
//! (`dogecoin:<network>:<instance>:scores`, member `[dataplane_id,
//! instance_id]`, score = indexed block height). Resolution picks the first
//! member, so requests always read from a registered — and therefore fully
//! backfilled — instance.

use lazy_static::lazy_static; // For global singleton
use r2d2::{Pool, PooledConnection};
use r2d2_redis2::redis::Commands;
use r2d2_redis2::RedisConnectionManager;
use redis::RedisResult;
use std::error::Error;
use std::fmt;
use std::sync::{Mutex, Once};
use strum_macros::EnumIter;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub network: Network,
    pub redis_address: String,
}

// Global configuration using lazy_static and Mutex for thread safety
lazy_static! {
    static ref CONFIG: Mutex<Option<AppConfig>> = Mutex::new(None);
}

// Helper function to initialize the configuration. This should only be called once.
static INIT: Once = Once::new();

#[derive(Debug)]
pub enum ConfigError {
    LockFailed(String),
}

impl Error for ConfigError {}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ConfigError::LockFailed(ref msg) => write!(f, "Lock failed: {}", msg),
        }
    }
}

pub fn initialize_config(network: Network, redis_address: String) -> Result<(), ConfigError> {
    INIT.call_once(|| {
        let config_result = CONFIG.lock();
        match config_result {
            Ok(mut config) => {
                *config = Some(AppConfig {
                    network,
                    redis_address,
                });
            }
            Err(poisoned) => {
                // Handle the poisoned lock by continuing with the locked value
                let mut config = poisoned.into_inner();
                *config = Some(AppConfig {
                    network,
                    redis_address,
                });
            }
        }
    });

    Ok(())
}

pub fn get_config() -> AppConfig {
    let config = CONFIG.lock().unwrap();
    config
        .clone()
        .expect("Configuration has not been initialized")
}

#[derive(Debug, Clone, Copy)]
pub enum Network {
    Mainnet,
    Testnet,
}

/// An *instance group*: the registry name under which a polyphony instance
/// running a group of reducers advertises itself.
#[derive(Debug, Clone, Copy)]
pub enum InstanceType {
    Inscriptions,
    InscriptionUtxosByScriptHash,
    ContentByInscriptionId,
    Dunes,
    SatBalanceByScriptHash,
    Scripts,
    Transactions,
    Utxos,
    BalancesByRuneId,
}

impl fmt::Display for InstanceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstanceType::Inscriptions => write!(f, "inscriptions"),
            InstanceType::InscriptionUtxosByScriptHash => {
                write!(f, "inscription-utxos-by-script-hash")
            }
            InstanceType::ContentByInscriptionId => {
                write!(f, "content-by-inscription-id")
            }
            InstanceType::Dunes => write!(f, "dunes"),
            InstanceType::SatBalanceByScriptHash => write!(f, "sat-balance-by-script-hash"),
            InstanceType::Scripts => write!(f, "scripts"),
            InstanceType::Transactions => write!(f, "transactions"),
            InstanceType::Utxos => write!(f, "utxos"),
            InstanceType::BalancesByRuneId => write!(f, "balances-by-rune-id"),
        }
    }
}

#[derive(Debug, Clone, Copy, EnumIter, PartialEq)]
pub enum ReducerType {
    BalancesByBrc20,
    Brc20BalancesByScriptHash,
    Brc20BalancesByTicker,
    Brc20TermsByTicker,
    ContentByInscriptionId,
    InscriptionsByUtxo,
    InscriptionUtxosByScriptHash,
    EtchingByRuneId,
    MintsByRuneId,
    RunesByUtxo,
    RuneBalancesByScriptHash,
    RuneIdByRuneName,
    SatBalanceByScriptHash,
    ScriptByScriptHash,
    ScriptHashByAddressPayloadHash,
    TransferInscriptionsByAddress,
    TxsByScriptHash,
    UtxosByRuneId,
    UtxosByScriptHash,
    BalancesByRuneId,
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Network::Mainnet => write!(f, "mainnet"),
            Network::Testnet => write!(f, "testnet"),
        }
    }
}

impl fmt::Display for ReducerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReducerType::InscriptionUtxosByScriptHash => write!(f, "InscriptionUtxosByScriptHash"),
            ReducerType::InscriptionsByUtxo => write!(f, "InscriptionsByUtxo"),
            ReducerType::ContentByInscriptionId => write!(f, "ContentByInscriptionId"),
            ReducerType::Brc20BalancesByScriptHash => write!(f, "Brc20BalancesByScriptHash"),
            ReducerType::Brc20TermsByTicker => write!(f, "Brc20TermsByTicker"),
            ReducerType::BalancesByBrc20 => write!(f, "BalancesByBrc20"),
            ReducerType::Brc20BalancesByTicker => write!(f, "Brc20BalancesByTicker"),
            ReducerType::UtxosByRuneId => write!(f, "UtxosByRuneId"),
            ReducerType::EtchingByRuneId => write!(f, "EtchingByRuneId"),
            ReducerType::MintsByRuneId => write!(f, "MintsByRuneId"),
            ReducerType::RunesByUtxo => write!(f, "RunesByUtxo"),
            ReducerType::RuneIdByRuneName => write!(f, "RuneIdByRuneName"),
            ReducerType::SatBalanceByScriptHash => write!(f, "SatBalanceByScriptHash"),
            ReducerType::ScriptByScriptHash => write!(f, "ScriptByScriptHash"),
            ReducerType::ScriptHashByAddressPayloadHash => {
                write!(f, "ScriptHashByAddressPayloadHash")
            }
            ReducerType::TransferInscriptionsByAddress => {
                write!(f, "TransferInscriptionsByAddress")
            }
            ReducerType::TxsByScriptHash => write!(f, "TxsByScriptHash"),
            ReducerType::UtxosByScriptHash => write!(f, "UtxosByScriptHash"),
            ReducerType::RuneBalancesByScriptHash => write!(f, "RuneBalancesByScriptHash"),
            ReducerType::BalancesByRuneId => write!(f, "BalancesByRuneId"),
        }
    }
}

// Create a global Redis connection pool using lazy_static
lazy_static! {
    static ref REDIS_POOL: Pool<RedisConnectionManager> = {
        let manager = RedisConnectionManager::new(get_config().redis_address)
            .expect("Failed to create Redis manager");
        Pool::builder()
            .build(manager)
            .expect("Failed to create Redis pool")
    };
}

fn get_redis_connection() -> RedisResult<PooledConnection<RedisConnectionManager>> {
    REDIS_POOL.get().map_err(|e| {
        redis::RedisError::from((
            redis::ErrorKind::IoError,
            "Failed to get Redis connection from pool",
            e.to_string(),
        ))
    })
}

#[derive(Debug)]
pub enum ResolveKeyError {
    RedisConnectionError(String),
    NoEntriesFound,
    EntryTooShort,
}

impl fmt::Display for ResolveKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ResolveKeyError::RedisConnectionError(ref e) => {
                write!(f, "Redis connection error: {}", e)
            }
            ResolveKeyError::NoEntriesFound => write!(f, "No entries found in the sorted set"),
            ResolveKeyError::EntryTooShort => write!(f, "Entry is too short"),
        }
    }
}

pub fn get_instance_for_reducer(reducer: ReducerType) -> InstanceType {
    match reducer {
        ReducerType::InscriptionUtxosByScriptHash => InstanceType::InscriptionUtxosByScriptHash,
        ReducerType::InscriptionsByUtxo => InstanceType::Inscriptions,
        ReducerType::ContentByInscriptionId => InstanceType::ContentByInscriptionId,
        ReducerType::Brc20BalancesByScriptHash => InstanceType::Inscriptions,
        ReducerType::Brc20TermsByTicker => InstanceType::Inscriptions,
        ReducerType::BalancesByBrc20 => InstanceType::Inscriptions,
        ReducerType::Brc20BalancesByTicker => InstanceType::Inscriptions,
        ReducerType::UtxosByRuneId => InstanceType::Dunes,
        ReducerType::EtchingByRuneId => InstanceType::Dunes,
        ReducerType::MintsByRuneId => InstanceType::Dunes,
        ReducerType::RunesByUtxo => InstanceType::Dunes,
        ReducerType::RuneIdByRuneName => InstanceType::Dunes,
        ReducerType::RuneBalancesByScriptHash => InstanceType::Dunes,
        ReducerType::SatBalanceByScriptHash => InstanceType::SatBalanceByScriptHash,
        ReducerType::ScriptByScriptHash => InstanceType::Scripts,
        ReducerType::ScriptHashByAddressPayloadHash => InstanceType::Scripts,
        ReducerType::TransferInscriptionsByAddress => InstanceType::Inscriptions,
        ReducerType::TxsByScriptHash => InstanceType::Transactions,
        ReducerType::UtxosByScriptHash => InstanceType::Utxos,
        ReducerType::BalancesByRuneId => InstanceType::BalancesByRuneId,
    }
}

/// Resolve the `(dataplane_id, instance_id)` whose keyspace should serve
/// queries for the given reducer.
pub fn resolve_key(reducer: ReducerType) -> Result<(u8, u8), ResolveKeyError> {
    let config = get_config();

    let instance = get_instance_for_reducer(reducer);

    // Construct the Redis key
    let key = format!("dogecoin:{}:{}:scores", config.network, instance);

    tracing::debug!("Retrieving key: {} from redis...", key);

    // Get a Redis connection from the pool
    let mut con =
        get_redis_connection().map_err(|e| ResolveKeyError::RedisConnectionError(e.to_string()))?;

    // Fetch the first entry from the sorted set in Redis
    let first_entry: Result<Vec<Vec<u8>>, _> = con.zrange(key, 0, 0);
    match first_entry {
        Ok(entries) => {
            if let Some(entry) = entries.first() {
                if entry.len() >= 2 {
                    let dataplane_id = entry[0]; // First byte is the Dataplane ID
                    let instance_id = entry[1]; // Second byte is the Instance ID
                    tracing::debug!(
                        "{} => dataplane_id: {} instance_id: {}",
                        reducer,
                        dataplane_id,
                        instance_id
                    );
                    Ok((dataplane_id, instance_id))
                } else {
                    Err(ResolveKeyError::EntryTooShort)
                }
            } else {
                Err(ResolveKeyError::NoEntriesFound)
            }
        }
        Err(e) => Err(ResolveKeyError::RedisConnectionError(e.to_string())),
    }
}
