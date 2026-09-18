mod address;
mod block;
pub mod dunes;
mod info;
pub mod inscriptions;
pub mod mempool_info;
pub mod mempool_transaction_ancestors;
pub mod mempool_transaction_descendants;
pub mod mempool_transaction_details;
pub mod mempool_transaction_fees;
pub mod mempool_transactions;
pub mod rpc;
mod transaction;
pub mod transaction_details;
mod transaction_result;
mod utxo;

pub mod dogecoin;

use std::{collections::HashMap, fmt::Debug};

use dunes::{AddressDuneUtxo, DuneHolder, DuneIdAndName, DuneInfo, DuneUtxo};
use inscriptions::{
    ContentBody, Drc20Holder, Drc20Info, Drc20Ticker, InscriptionByAddress, InscriptionInfo,
    TransferInscriptionByAddress,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub use self::{address::*, block::*, info::*, transaction::*, transaction_result::*, utxo::*};

pub type BlockHeight = u64;

#[derive(Clone, Copy, Debug, Deserialize, ToSchema, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[schema(default = "asc")]
pub enum OrderParam {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, Deserialize, ToSchema, PartialEq, PartialOrd)]
#[schema(default = 100)]
pub struct CountParam(pub usize);

#[derive(Debug, Deserialize, ToSchema)]
pub struct CursorPaginationParams {
    pub count: Option<CountParam>,
    pub order: Option<OrderParam>,
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct HeightPaginationParams {
    pub count: Option<CountParam>,
    pub order: Option<OrderParam>,
    pub cursor: Option<String>,
    pub from: Option<u64>, // inclusive
    pub to: Option<u64>,   // inclusive
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct LastUpdated {
    pub block_hash: String,
    pub block_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[aliases(
    PaginatedAddressDuneUtxo = PaginatedResponse<AddressDuneUtxo>,
    PaginatedDuneHolder = PaginatedResponse<DuneHolder>,
    PaginatedDuneIdAndName = PaginatedResponse<DuneIdAndName>,
    PaginatedDuneUtxo = PaginatedResponse<DuneUtxo>,
    PaginatedDrc20Holder = PaginatedResponse<Drc20Holder>,
    PaginatedDrc20Ticker = PaginatedResponse<Drc20Ticker>,
    PaginatedInscriptionByAddress = PaginatedResponse<InscriptionByAddress>,
    PaginatedInvolvedTransaction = PaginatedResponse<InvolvedTransaction>,
    PaginatedTransferInscriptionByAddress = PaginatedResponse<TransferInscriptionByAddress>,
    PaginatedUtxo = PaginatedResponse<Utxo>,
)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub last_updated: LastUpdated,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[aliases(
    TimestampedDrc20Quantities = TimestampedResponse<HashMap<String, String>>,
    TimestampedDrc20Info = TimestampedResponse<Drc20Info>,
    TimestampedDuneQuantities = TimestampedResponse<HashMap<String, String>>,
    TimestampedDuneInfo = TimestampedResponse<DuneInfo>,
    TimestampedInscriptionInfo = TimestampedResponse<InscriptionInfo>,
    TimestampedTotalBalanceByAddress = TimestampedResponse<String>
)]
pub struct TimestampedResponse<T> {
    pub data: T,
    pub last_updated: LastUpdated,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[aliases(
    PaginatedContentBody = PaginatedContent<ContentBody>,
)]
pub struct PaginatedContent<T> {
    pub data: T,
    pub last_updated: LastUpdated,
    pub next_cursor: Option<String>,
}
