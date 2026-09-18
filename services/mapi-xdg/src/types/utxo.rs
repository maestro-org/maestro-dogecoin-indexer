use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct Utxo {
    pub txid: String,
    pub vout: u32,
    pub address: Option<String>,
    pub script_pubkey: String,
    pub satoshis: String,
    pub confirmations: u64,
    pub height: u64,
    pub dunes: Vec<DuneAndAmount>,
    pub inscriptions: Vec<InscriptionAndOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct DuneAndAmount {
    pub dune_id: String,
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct InscriptionAndOffset {
    pub offset: u32,
    pub inscription_id: String,
}
