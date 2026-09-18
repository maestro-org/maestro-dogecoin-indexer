use inscription_id::InscriptionId;
use serde::{Deserialize, Serialize};

use super::{DBBytes, DBHash32, DBSerde, DBUInt128, KVTable};

pub mod brc20;
pub mod inscription;
pub mod inscription_id;
pub mod updater;

// Txid -> Concatenation of txids involved in partial inscription so far
pub struct PartialTxidToTxidsKV;

impl KVTable<DBHash32, DBBytes> for PartialTxidToTxidsKV {
    const CF_NAME: &'static str = "PartialTxidToTxidsKV";
}

// Txid -> Concatenation of txids involved in partial inscription so far
pub struct PartialTxidToBytesKV;

impl KVTable<DBHash32, DBBytes> for PartialTxidToBytesKV {
    const CF_NAME: &'static str = "PartialTxidToBytesKV";
}

// INSCRIPTION_COUNTERS_KEY -> current counters needed for inscriptions
pub struct InscriptionCountersKV;

impl KVTable<DBBytes, DBSerde<Counters>> for InscriptionCountersKV {
    const CF_NAME: &'static str = "InscriptionCountersKV";
}

pub static INSCRIPTION_COUNTERS_KEY: &[u8] = &[0x14];

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Counters {
    cursed_count: u64,
    blessed_count: u64,
    next_sequence_num: u64,
    lost_sats: u64,
    unbound_count: u64,
}

//
pub struct TermsByBRC20Ticker;

impl KVTable<DBBytes, DBSerde<BRC20Terms>> for TermsByBRC20Ticker {
    const CF_NAME: &'static str = "TermsByBRC20TickerKV";
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BRC20Terms {
    max: u128,
    mint_amt_limit: u128,
    dec: u8,
    self_mint: bool,
    deploy_id: InscriptionId,
}

// BRC20 ticker -> current minted supply
pub struct SupplyByBRC20;

impl KVTable<DBBytes, DBUInt128> for SupplyByBRC20 {
    const CF_NAME: &'static str = "SupplyByBRC20KV";
}

// (script hash, brc20 ticker) -> available balance
pub struct BRC20Balances;

impl KVTable<DBSerde<ScriptAndBRC20Kind>, DBUInt128> for BRC20Balances {
    const CF_NAME: &'static str = "BRC20BalancesKV";
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ScriptAndBRC20Kind {
    pub script: [u8; 20],
    pub brc20_ticker: Vec<u8>,
}
