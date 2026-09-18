use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct DuneInfo {
    pub id: String,
    pub etching_cenotaph: bool,
    pub etching_tx: String,
    pub etching_height: u64,
    pub name: String,
    pub spaced_name: String,
    pub symbol: Option<char>,
    /// If no divisibility was specified, then this equals 0
    pub divisibility: u8,
    pub terms: Terms,
    pub max_supply: Option<String>,
    pub circulating_supply: String,
    pub mints: u64,
    pub unique_holders: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct Terms {
    pub mint_txs_cap: Option<String>,
    pub amount_per_mint: Option<String>,
    pub start_height: Option<String>,
    pub end_height: Option<String>,
    pub start_offset: Option<String>,
    pub end_offset: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct DuneUtxo {
    pub txid: String,
    pub vout: u32,
    pub address: Option<String>,
    pub script_pubkey: String,
    pub satoshis: String,
    pub confirmations: u64,
    pub height: u64,
    pub dune_amount: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct AddressDuneUtxo {
    pub txid: String,
    pub vout: u32,
    pub satoshis: String,
    pub confirmations: u64,
    pub height: u64,
    pub dune_amount: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct DuneIdAndName {
    pub id: String,
    pub spaced_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, Hash)]
pub struct DuneHolder {
    pub address: Option<String>,
    pub script_pubkey: String,
    pub balance: String,
}
