use serde::{Deserialize, Serialize};

use super::{DBBytes, DBInt, DBSerde, DBUInt128, DuneId, KVTable};

pub mod dunes;
pub mod updater;

// Rune name -> Rune ID
pub struct DuneIdByNameKV;

impl KVTable<DBUInt128, DBSerde<DuneId>> for DuneIdByNameKV {
    const CF_NAME: &'static str = "DuneIdByNameKV";
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DuneTerms {
    pub name: u128,
    pub amount: Option<u128>,
    pub cap: Option<u128>,
    pub start_height: Option<u64>,
    pub end_height: Option<u64>,
}

// Rune ID -> Rune Terms
pub struct DuneTermsByIdKV;

impl KVTable<DuneId, DBSerde<DuneTerms>> for DuneTermsByIdKV {
    const CF_NAME: &'static str = "DuneTermsByIdKV";
}

// Rune ID -> Number of times rune minted
pub struct DuneMintsByIdKV;

impl KVTable<DuneId, DBUInt128> for DuneMintsByIdKV {
    const CF_NAME: &'static str = "DuneMintsByIdKV";
}

// DUNES_RESERVED_COUNTERS_KEY -> current counters needed for inscriptions
pub struct DunesReservedCountersKV;

impl KVTable<DBBytes, DBInt> for DunesReservedCountersKV {
    const CF_NAME: &'static str = "DunesReservedCountersKV";
}

pub static DUNES_RESERVED_COUNTERS_KEY: &[u8] = &[0x15];
